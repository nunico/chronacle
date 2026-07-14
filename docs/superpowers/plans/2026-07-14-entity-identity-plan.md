# Entity Identity Implementation Plan (Tranche 6)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give Chronacle a concept of entity identity beyond an exact name string — aliases ("alternate names"), tiered wikilink resolution, fuzzy duplicate detection, a real merge operation, and vault keys that survive a rename.

**Architecture:** One new GM-owned field, `aliases: array<string>`, is the single identity primitive. Everything else either **populates** it (fuzzy auto-resolve, confirm-a-suggestion, merge) or **honors** it (link resolution, duplicate detection). A pure, dependency-free `naming` module (normalize + trigram similarity) is the shared engine for both resolution and duplicate detection. Rename safety lands before merge, because merge renames by definition.

**Tech Stack:** Rust (SurrealDB embedded, SurrealQL only), Svelte 5 runes, Tauri IPC. **No new crates** — similarity is hand-rolled.

**Spec:** [`docs/superpowers/specs/2026-07-14-entity-identity-design.md`](../specs/2026-07-14-entity-identity-design.md)

## Global Constraints

- **No new dependencies.** Similarity is hand-rolled trigram Dice. A new `Cargo.toml` entry requires an ADR and an architecture-doc table entry (project hard constraint).
- **SurrealQL only.** No SQL. Schema via `DEFINE` statements in `.surql` files. `run_migrations` re-runs every file on every boot, so **migrations must be DEFINE-only and idempotent** — never `REMOVE`.
- **`DEFAULT` never backfills, and an unset field breaks every WRITE** to a pre-migration row. Any `DEFINE FIELD` added here **must** also be added to `backfill_unset_fields` in `crates/chronacle-db/src/schema/mod.rs`, in the same task.
- **Soft-delete filter is `!= true`, never `= false`** (a pre-migration row holds `NONE`, and `NONE = false` is false).
- **Traits for all external deps.** `chronacle-vault` stays filesystem-free — go through `VaultStore`.
- **IDs are bare** in the vault layer (`"n1"`, not `"npc:n1"`); `VaultRef { table, id }`, `to_thing()` → `table:id`.
- **GM-facing term is "alternate names"**, never "aliases". `aliases` is the code/frontmatter identifier only.
- **Svelte 5 runes only** (`$state`, `$derived`, `$props`, `$effect`). No `export let`, no `$:`.
- **Tests ship in the same task as the feature.** Every user-visible behavior adds `.feature` scenarios (ADR-011).
- **Clippy warnings are errors:** `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- Commit subjects ≤ 72 chars, imperative mood.

## File Structure

| File                                                        | Responsibility                                                                                                     |
| ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `crates/chronacle-db/src/schema/004_entity_identity.surql`  | **Create.** `aliases` on 8 entity tables + `rule_entry`; new lint kinds documented.                                |
| `crates/chronacle-db/src/schema/mod.rs`                     | **Modify.** Register the migration; add `aliases` to `backfill_unset_fields`.                                      |
| `crates/chronacle-extraction/src/naming.rs`                 | **Create.** Pure: `normalize()`, `similarity()`, `best_match()`. No I/O. The engine for both resolution and dedup. |
| `crates/chronacle-extraction/src/wikilink/query.rs`         | **Modify.** Fetch `aliases` alongside `name`, scope-aware.                                                         |
| `crates/chronacle-extraction/src/wikilink/resolve.rs`       | **Create.** The 4-tier resolver, extracted from `mod.rs`'s inline `filter_map`.                                    |
| `crates/chronacle-extraction/src/wikilink/mod.rs`           | **Modify.** Call the resolver; emit auto-aliases + candidates.                                                     |
| `crates/chronacle-extraction/src/entity_service/aliases.rs` | **Create.** Alias validation (scope collision) + add/remove.                                                       |
| `crates/chronacle-extraction/src/entity_service/merge.rs`   | **Create.** `merge()` — edge union, alias union, field choices, soft-delete.                                       |
| `crates/chronacle-extraction/src/codex_service/lint.rs`     | **Modify.** Fuzzy `lint_duplicates`; `alias_collision`; `broken_wikilink` candidates.                              |
| `crates/chronacle-vault/src/reconcile.rs`                   | **Modify.** Key-move detection (rename safety).                                                                    |
| `crates/chronacle-vault/src/render.rs`                      | **Modify.** Frontmatter `aliases` = `[name] ∪ aliases`.                                                            |
| `crates/chronacle-vault/src/frontmatter.rs`                 | **Modify.** Inbound GM aliases = `frontmatter.aliases − name`.                                                     |
| `crates/chronacle-domain/src/campaign_service.rs`           | **Modify.** `rename()`.                                                                                            |
| `apps/desktop/src-tauri/src/commands/entity_commands.rs`    | **Modify.** `merge_entities`, `set_entity_aliases`.                                                                |
| `apps/desktop/src-tauri/src/commands/lint_commands.rs`      | **Modify.** `confirm_alias_suggestion`, `undo_auto_alias`.                                                         |
| `apps/desktop/src/components/AliasField.svelte`             | **Create.** "Alternate names" chips.                                                                               |
| `apps/desktop/src/components/MergeDialog.svelte`            | **Create.** Side-by-side field-by-field merge.                                                                     |
| `apps/desktop/src/views/MaintenanceView.svelte`             | **Modify.** Did-you-mean, Merge, Auto-linked, collisions.                                                          |
| `docs/user-guide.md`                                        | **Modify.** "Names and duplicates" chapter (copy is drafted in the spec).                                          |
| `docs/architecture.md`                                      | **Modify.** ADR-012.                                                                                               |

---

### Task 1 (F1): `aliases` field, schema, backfill, and read/write plumbing

Foundation. Nothing else can be built until an entity can _hold_ an alternate name.

**Files:**

- Create: `crates/chronacle-db/src/schema/004_entity_identity.surql`
- Modify: `crates/chronacle-db/src/schema/mod.rs`
- Modify: `crates/chronacle-extraction/src/entity_service/types.rs` (`EntityInput`, `GraphNode`)
- Modify: `crates/chronacle-extraction/src/entity_service/crud/write.rs`, `crud/update.rs`, `crud/read.rs`
- Test: `apps/desktop/src-tauri/tests/entity_aliases_test.rs`

**Interfaces:**

- Produces: `EntityInput.aliases: Vec<String>`; `GraphNode.aliases: Vec<String>`; schema field `aliases` on `npc`, `location`, `faction`, `creature`, `item`, `event`, `player_character`, `misc`, `rule_entry`.

- [ ] **Step 1: Write the failing test**

`apps/desktop/src-tauri/tests/entity_aliases_test.rs`:

```rust
//! `aliases` must round-trip through create/update/read — AND must not break
//! writes to rows that predate the migration (the DEFAULT landmine).

use chronacle_extraction::entity_service::{self, EntityInput, EntityKind};

async fn db() -> surrealdb::Surreal<surrealdb::engine::any::Any> {
    let db = surrealdb::engine::any::connect("mem://").await.unwrap();
    db.use_ns("t").use_db("t").await.unwrap();
    db
}

#[tokio::test]
async fn aliases_round_trip_through_create_and_read() {
    let db = db().await;
    chronacle_db::run_migrations(&db).await.expect("migrations");
    db.query("CREATE campaign:c1 SET name = 'SoV', system = '5e', \
              created_at = time::now(), updated_at = time::now()")
        .await.unwrap().check().unwrap();

    let input = EntityInput {
        name: "The Quassar Family".to_string(),
        aliases: vec!["The Quassars".to_string(), "Quassar Clan".to_string()],
        ..Default::default()
    };
    let node = entity_service::create(&db, EntityKind::Faction, input, "c1", None)
        .await
        .expect("create");

    let read = entity_service::get_by_id(&db, &node.id, EntityKind::Faction)
        .await
        .expect("read");
    assert_eq!(read.aliases, vec!["The Quassars", "Quassar Clan"]);
}

/// THE LANDMINE TEST. `DEFINE FIELD ... DEFAULT []` is a WRITE-time default: it
/// never touches rows that already exist. SurrealDB re-validates EVERY field of
/// a SCHEMAFULL record on ANY write, and `NONE` does not satisfy
/// `array<string>` — so a single unset field makes every LATER write to that
/// row fail. Seeding BEFORE run_migrations is the only way to reproduce a real
/// user's pre-migration row; fresh fixtures pick up the DEFAULT and are blind
/// to this. (Tranche 5 shipped this bug green.)
#[tokio::test]
async fn a_pre_migration_row_can_still_be_written_to() {
    let db = db().await;

    // Seeded BEFORE migrations: this row has no `aliases` value at all.
    db.query("DEFINE TABLE npc SCHEMALESS; \
              CREATE npc:old SET name = 'Seraphina';")
        .await.unwrap().check().unwrap();

    chronacle_db::run_migrations(&db).await.expect("migrations");

    // The write that would fail with: Found NONE for field `aliases`,
    // with record `npc:old`, but expected a array<string>
    db.query("UPDATE npc:old SET notes = 'edited'")
        .await
        .expect("query")
        .check()
        .expect("a pre-migration row must still accept writes after migrating");
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p Chronacle --test entity_aliases_test`
Expected: FAIL — `EntityInput` has no field `aliases` (compile error).

- [ ] **Step 3: Add the schema migration**

Create `crates/chronacle-db/src/schema/004_entity_identity.surql`:

```surql
-- ── Tranche 6: entity identity ───────────────────────────────────────────────
-- `aliases` — GM-owned alternate names. A name variant is not a new entity; it
-- is another name for an existing one. Populated by merge, by confirming a
-- suggestion, by fuzzy auto-resolve, or by the GM directly (in-app or in the
-- vault frontmatter). Honored by wikilink resolution and duplicate detection.
--
-- DEFINE-only and idempotent: run_migrations re-runs every .surql on EVERY
-- boot. Never add a REMOVE here (one once wiped every relationship edge).

DEFINE FIELD OVERWRITE aliases ON TABLE npc              TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE aliases ON TABLE location         TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE aliases ON TABLE faction          TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE aliases ON TABLE creature         TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE aliases ON TABLE item             TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE aliases ON TABLE event            TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE aliases ON TABLE player_character TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE aliases ON TABLE misc             TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE aliases ON TABLE rule_entry       TYPE array<string> DEFAULT [];

-- New lint_finding kinds (payload shapes; the table itself is unchanged):
--   alias_collision : { alias, a, b }
--     Two entities in the same resolution scope claim the same name/alias.
--   auto_alias      : { entity, alias, similarity, source }
--     An alias written by tier-4 fuzzy auto-resolve. Informational; undoable.
-- And `broken_wikilink` gains an optional `candidates: [{id, name, similarity}]`.
```

Register it in `crates/chronacle-db/src/schema/mod.rs` alongside the other three (follow the existing `include_str!` list exactly).

- [ ] **Step 4: Add `aliases` to the backfill — the same task, not later**

In `crates/chronacle-db/src/schema/mod.rs`, extend `ENTITY_SET`:

```rust
    const ENTITY_SET: &str = "summary       = summary       ?? NULL, \
         notes         = notes         ?? NULL, \
         embedding     = embedding     ?? NULL, \
         embed_model   = embed_model   ?? NULL, \
         codex_stale   = codex_stale   ?? false, \
         codex_sources = codex_sources ?? [], \
         vault_deleted = vault_deleted ?? false, \
         aliases       = aliases       ?? [], \
         created_at    = created_at    ?? time::now(), \
         updated_at    = updated_at    ?? time::now()";
```

and extend the `rule_entry` statement:

```rust
    statements.push(
        "UPDATE rule_entry SET vault_deleted = vault_deleted ?? false, aliases = aliases ?? []"
            .to_owned(),
    );
```

`??` coalesces only NONE/NULL, so this is idempotent. It stays **non-fatal** (log and continue): a backfill that cannot run must never stop the app from opening its database.

- [ ] **Step 5: Plumb `aliases` through the entity service**

`types.rs` — add to `EntityInput` (after `notes`) and to `GraphNode`:

```rust
    #[serde(default)]
    pub aliases: Vec<String>,
```

`crud/write.rs` (`create`) and `crud/update.rs` (`update`) — bind it. Aliases are a plain `array<string>`, never NULL, so bind directly (no `opt_value`):

```rust
        .bind(("aliases", input.aliases.clone()))
```

and add `aliases = $aliases` to the `SET` clause of both statements. `crud/read.rs` — add `aliases` to every `SELECT` field list that builds a `GraphNode`.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p Chronacle --test entity_aliases_test`
Expected: PASS (2 tests).

Run: `cargo test --workspace`
Expected: all green — the new field is additive.

- [ ] **Step 7: Commit**

```bash
git add crates/chronacle-db crates/chronacle-extraction apps/desktop/src-tauri/tests/entity_aliases_test.rs
git commit -m "feat(identity): aliases field on entities and rule entries"
```

---

### Task 2 (F2): The `naming` module — normalize + trigram similarity

Pure, no I/O, no DB. The shared engine for resolution (Task 3/4) and duplicate detection (Task 5). Exhaustively unit-tested because everything downstream trusts it.

**Files:**

- Create: `crates/chronacle-extraction/src/naming.rs`
- Modify: `crates/chronacle-extraction/src/lib.rs` (`pub mod naming;`)

**Interfaces:**

- Produces:
  - `pub fn normalize(name: &str) -> String`
  - `pub fn similarity(a: &str, b: &str) -> f64` — 0.0..=1.0, operates on **already-normalized** input
  - `pub fn best_match<'a>(needle: &str, haystack: &'a [(String, String)], threshold: f64) -> MatchOutcome<'a>` where `haystack` is `(id, name)`
  - `pub enum MatchOutcome<'a> { None, Unique { id: &'a str, name: &'a str, score: f64 }, Ambiguous(Vec<Candidate>) }`
  - `pub struct Candidate { pub id: String, pub name: String, pub similarity: f64 }`
  - `pub const DEFAULT_THRESHOLD: f64` — provisional 0.72, **tuned against real data in Task 10**

- [ ] **Step 1: Write the failing tests**

In `crates/chronacle-extraction/src/naming.rs` (`#[cfg(test)] mod tests`):

```rust
    #[test]
    fn normalize_collapses_the_article_and_plural_variants() {
        // The maintainer's real cases.
        assert_eq!(normalize("The Free League"), normalize("Free League"));
        assert_eq!(normalize("The Quassars"), "quassar");
        assert_eq!(normalize("The Quassar Family"), "quassar family");
        assert_eq!(normalize("Seraphina's Blade"), "seraphina blade");
    }

    #[test]
    fn normalize_does_not_over_collapse() {
        // Indefinite articles are NOT stripped: too many distinct titles start
        // with "A" for that to be safe.
        assert_eq!(normalize("A Cage of Iron"), "a cage of iron");
        // A trailing "s" that is not a plural must survive.
        assert_eq!(normalize("Chaos"), "chaos");
        // "the" only leads; it is not stripped mid-name.
        assert_eq!(normalize("Lord of the Rings"), "lord of the ring");
    }

    #[test]
    fn normalize_is_idempotent() {
        for s in ["The Quassars", "Chaos", "  The   Free  League  ", "Ünther's"] {
            assert_eq!(normalize(&normalize(s)), normalize(s), "input: {s}");
        }
    }

    #[test]
    fn similarity_scores_a_partial_name_high_and_a_stranger_low() {
        let quassars = normalize("The Quassars");
        let family = normalize("The Quassar Family");
        assert!(similarity(&quassars, &family) > 0.7, "partial name must score high");

        // NEGATIVE CASE — these must NOT match. A faction and a tavern.
        // A false merge here is data loss, so this assertion matters more
        // than any positive one.
        let legion = normalize("The Legion");
        let rest = normalize("The Legionnaire's Rest");
        assert!(similarity(&legion, &rest) < 0.72, "distinct entities must not match");
    }

    #[test]
    fn best_match_refuses_to_guess_when_two_candidates_tie() {
        let haystack = vec![
            ("faction:a".to_string(), "The Quassar Family".to_string()),
            ("faction:b".to_string(), "The Quassar Cartel".to_string()),
        ];
        match best_match("the quassars", &haystack, 0.5) {
            MatchOutcome::Ambiguous(c) => assert_eq!(c.len(), 2),
            other => panic!("ambiguity must never auto-resolve, got {other:?}"),
        }
    }
```

- [ ] **Step 2: Run and watch them fail**

Run: `cargo test -p chronacle-extraction naming`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement**

```rust
//! Pure name matching: normalization + trigram similarity. No I/O, no DB.
//!
//! Shared by wikilink resolution and duplicate detection so the two can never
//! disagree about whether two names are "the same".
//!
//! English-centric by design (leading "the", trailing plural "s"): the corpus
//! is English TTRPG material. Rules can grow here without touching callers.

/// Ranked near-match.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub id: String,
    pub name: String,
    pub similarity: f64,
}

/// The result of a fuzzy lookup. `Ambiguous` is NOT a failure — it is a refusal
/// to guess, and it is what the "did you mean …?" suggestion is built from.
#[derive(Debug, Clone, PartialEq)]
pub enum MatchOutcome<'a> {
    None,
    Unique { id: &'a str, name: &'a str, score: f64 },
    Ambiguous(Vec<Candidate>),
}

/// Provisional. Tuned against real campaign data in Task 10 and recorded in
/// ADR-012 with the evidence. Prefer a MISSED match (degrades to a suggestion)
/// over a FALSE one (silently corrupts the graph).
pub const DEFAULT_THRESHOLD: f64 = 0.72;

/// Case-fold, strip a leading "the", drop possessives, singularize a trailing
/// plural, collapse punctuation and whitespace. Never used for storage — only
/// ever as a lookup key. The GM's exact spelling is preserved as name or alias.
pub fn normalize(name: &str) -> String {
    let lowered = name.to_lowercase();
    let cleaned: String = lowered
        .chars()
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
        .collect();

    let words: Vec<&str> = cleaned.split_whitespace().collect();
    let words = match words.split_first() {
        Some((&"the", rest)) if !rest.is_empty() => rest.to_vec(),
        _ => words,
    };

    words
        .iter()
        .map(|w| singularize(w))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Conservative: a small rule set, not a stemmer. Over-eager singularization
/// merges distinct names, which is the expensive direction to be wrong in.
fn singularize(word: &str) -> String {
    // Possessives were already stripped to a bare "s" by punctuation removal
    // ("seraphina's" -> "seraphina s"), so a lone "s" is dropped by the
    // whitespace collapse and never reaches here.
    if word.len() > 3 && word.ends_with("ies") {
        return format!("{}y", &word[..word.len() - 3]);
    }
    if word.len() > 3 && word.ends_with("es") && !word.ends_with("ses") {
        return word[..word.len() - 2].to_string();
    }
    // "chaos", "ss" endings, and short words keep their "s".
    if word.len() > 3 && word.ends_with('s') && !word.ends_with("ss") && !word.ends_with("us") {
        return word[..word.len() - 1].to_string();
    }
    word.to_string()
}

fn trigrams(s: &str) -> Vec<[char; 3]> {
    let padded: Vec<char> = format!("  {s} ").chars().collect();
    padded.windows(3).map(|w| [w[0], w[1], w[2]]).collect()
}

/// Dice coefficient over character trigrams, plus a containment bonus so a
/// short name scores high against a longer one that contains it
/// ("quassar" vs "quassar family") — which is exactly the elided-link case.
/// Both inputs must already be normalized.
pub fn similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let (ta, tb) = (trigrams(a), trigrams(b));
    let shared = ta.iter().filter(|t| tb.contains(t)).count();
    let dice = (2.0 * shared as f64) / (ta.len() + tb.len()) as f64;

    // Whole-word containment: every word of the shorter name appears in the
    // longer one. "quassar" ⊂ "quassar family" -> strong signal.
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let long_words: Vec<&str> = long.split_whitespace().collect();
    let contained = short.split_whitespace().all(|w| long_words.contains(&w));

    if contained {
        dice.max(0.75 + 0.25 * dice)
    } else {
        dice
    }
}

/// Find the single best match above `threshold`. Returns `Ambiguous` when more
/// than one candidate clears it — the caller MUST NOT pick a winner.
pub fn best_match<'a>(
    needle: &str,
    haystack: &'a [(String, String)],
    threshold: f64,
) -> MatchOutcome<'a> {
    let n = normalize(needle);
    let mut hits: Vec<(f64, &'a str, &'a str)> = haystack
        .iter()
        .map(|(id, name)| (similarity(&n, &normalize(name)), id.as_str(), name.as_str()))
        .filter(|(score, _, _)| *score >= threshold)
        .collect();

    hits.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    match hits.len() {
        0 => MatchOutcome::None,
        1 => MatchOutcome::Unique { id: hits[0].1, name: hits[0].2, score: hits[0].0 },
        _ => MatchOutcome::Ambiguous(
            hits.into_iter()
                .map(|(similarity, id, name)| Candidate {
                    id: id.to_string(),
                    name: name.to_string(),
                    similarity,
                })
                .collect(),
        ),
    }
}
```

Add `pub mod naming;` to `crates/chronacle-extraction/src/lib.rs`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p chronacle-extraction naming`
Expected: PASS (5 tests). If `similarity("legion", "legionnaire rest")` lands above 0.72, tighten the containment bonus rather than raising the threshold — the negative case is the one that must hold.

- [ ] **Step 5: Commit**

```bash
git add crates/chronacle-extraction/src/naming.rs crates/chronacle-extraction/src/lib.rs
git commit -m "feat(identity): pure name normalization and trigram similarity"
```

---

### Task 3 (F3): Tiers 1–3 — exact, alias, normalized

Deterministic tiers only. No scoring, no threshold, no ambiguity. This alone fixes "The Free League" / "Free League".

**Files:**

- Create: `crates/chronacle-extraction/src/wikilink/resolve.rs`
- Modify: `crates/chronacle-extraction/src/wikilink/query.rs`, `mod.rs`
- Test: in `resolve.rs` + `crates/chronacle-extraction/src/wikilink/wikilink_tests_extra.rs`

**Interfaces:**

- Consumes: `naming::normalize` (Task 2); `aliases` field (Task 1).
- Produces:
  - `query_all_entity_names` now returns `Vec<EntityIdentity>` where
    `pub struct EntityIdentity { pub id: String, pub name: String, pub aliases: Vec<String> }`
  - `pub fn resolve_exact(link: &str, entities: &[EntityIdentity]) -> Option<String>` — tiers 1–3, returns full record id.

- [ ] **Step 1: Write the failing test**

In `resolve.rs`:

```rust
    fn fixture() -> Vec<EntityIdentity> {
        vec![
            EntityIdentity {
                id: "faction:fl".into(),
                name: "The Free League".into(),
                aliases: vec![],
            },
            EntityIdentity {
                id: "npc:s".into(),
                name: "Seraphina Aldric".into(),
                aliases: vec!["Sera".into()],
            },
        ]
    }

    #[test]
    fn tier_1_exact_name_still_wins() {
        assert_eq!(resolve_exact("the free league", &fixture()).as_deref(), Some("faction:fl"));
    }

    #[test]
    fn tier_2_matches_a_confirmed_alias() {
        assert_eq!(resolve_exact("Sera", &fixture()).as_deref(), Some("npc:s"));
    }

    #[test]
    fn tier_3_matches_across_the_leading_article() {
        // The maintainer's case: the notes say "Free League", the entity is
        // "The Free League". Both normalize to "free league" -> EXACT match on
        // a normalized key. No threshold involved.
        assert_eq!(resolve_exact("Free League", &fixture()).as_deref(), Some("faction:fl"));
    }

    #[test]
    fn an_unrelated_name_does_not_resolve() {
        assert_eq!(resolve_exact("Iron Host", &fixture()), None);
    }
```

- [ ] **Step 2: Run and watch it fail**

Run: `cargo test -p chronacle-extraction wikilink::resolve`
Expected: FAIL — `resolve_exact` not defined.

- [ ] **Step 3: Implement the resolver**

`crates/chronacle-extraction/src/wikilink/resolve.rs`:

```rust
//! Tiered wikilink resolution. Tiers 1-3 are deterministic (this file); tier 4
//! (fuzzy) lives in `mod.rs` because it has side effects — it writes an alias.

use crate::naming::normalize;

/// An entity as the resolver sees it: identity plus every name it answers to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityIdentity {
    /// Full record id, e.g. `"npc:abc123"`.
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
}

/// Tiers 1-3, in order, first hit wins:
///   1. exact name (case-insensitive)
///   2. exact alias (case-insensitive)   — a confirmed variant, forever
///   3. normalized name or alias         — "Free League" == "The Free League"
///
/// Returns the full record id. A tier-3 match is still EXACT — on a normalized
/// key — so there is no threshold and no ambiguity to adjudicate here.
pub fn resolve_exact(link: &str, entities: &[EntityIdentity]) -> Option<String> {
    let lower = link.trim().to_lowercase();

    if let Some(e) = entities.iter().find(|e| e.name.to_lowercase() == lower) {
        return Some(e.id.clone());
    }
    if let Some(e) = entities
        .iter()
        .find(|e| e.aliases.iter().any(|a| a.to_lowercase() == lower))
    {
        return Some(e.id.clone());
    }

    let norm = normalize(link);
    if norm.is_empty() {
        return None;
    }
    entities
        .iter()
        .find(|e| {
            normalize(&e.name) == norm || e.aliases.iter().any(|a| normalize(a) == norm)
        })
        .map(|e| e.id.clone())
}
```

- [ ] **Step 4: Make `query_all_entity_names` return aliases**

In `query.rs`, change both scope branches to `SELECT id, name, aliases FROM {table} …`, change `EntityNameRow` to carry `aliases: Vec<String>` (with `#[serde(default)]`), and return `Vec<EntityIdentity>`. **Do not touch the WHERE clauses** — the scope traversal (campaign `in_campaign` + `subscribes_to->in_collection`; collection `in_collection` only) is what stops a link in one campaign resolving to another campaign's entity, and aliases must honor exactly the same scope.

In `mod.rs`, replace the inline `filter_map` (the `name.to_lowercase() == lower` comparison) with `resolve::resolve_exact(wikilink_name, &all_entities)`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p chronacle-extraction wikilink`
Expected: PASS — including every pre-existing wikilink test (tier 1 is unchanged behavior).

- [ ] **Step 6: Commit**

```bash
git add crates/chronacle-extraction/src/wikilink
git commit -m "feat(identity): resolve wikilinks by alias and normalized name"
```

---

### Task 4 (F4): Tier 4 — fuzzy auto-resolve, auto-alias, and did-you-mean candidates

Where trust is won or lost. Auto-resolve **only when unambiguous**, persist what it decides, and surface it.

**Files:**

- Modify: `crates/chronacle-extraction/src/wikilink/mod.rs`
- Create: `crates/chronacle-extraction/src/entity_service/aliases.rs`
- Modify: `crates/chronacle-extraction/src/codex_service/lint.rs`
- Test: `apps/desktop/src-tauri/tests/wikilink_fuzzy_test.rs`

**Interfaces:**

- Consumes: `naming::{best_match, MatchOutcome, Candidate, DEFAULT_THRESHOLD}`; `resolve_exact`.
- Produces:
  - `entity_service::aliases::add_alias(db, full_id, alias) -> Result<(), EntityError>` — validates scope collision, appends.
  - `entity_service::aliases::remove_alias(db, full_id, alias) -> Result<(), EntityError>`
  - `lint_finding` kinds `auto_alias` and `alias_collision`; `broken_wikilink.payload.candidates`.

- [ ] **Step 1: Write the failing test**

`apps/desktop/src-tauri/tests/wikilink_fuzzy_test.rs`:

```rust
//! Tier 4: an elided link auto-resolves ONLY when there is exactly one
//! sensible answer — and it leaves a trace when it does.

use chronacle_extraction::wikilink::{parse_and_sync_wikilinks, WikilinkScope};

#[tokio::test]
async fn an_elided_link_auto_resolves_and_persists_the_alias() {
    let db = seeded_db().await; // faction:q "The Quassar Family", npc:s "Seraphina"

    let matched = parse_and_sync_wikilinks(
        &db, "npc", "s", "Met [[The Quassars]] today.",
        WikilinkScope::Campaign { campaign_id: "c1" },
    ).await.expect("resolve");

    assert_eq!(matched, vec!["faction:q"], "the elided link must find the family");

    // It must have REMEMBERED. The next pass hits tier 2, not the fuzzy path:
    // fuzzy runs once per variant, ever.
    let aliases: Vec<String> = db
        .query("SELECT VALUE aliases FROM faction:q").await.unwrap()
        .take::<Vec<Vec<String>>>(0).unwrap().remove(0);
    assert!(aliases.iter().any(|a| a == "The Quassars"));

    // And it must be REVIEWABLE — nothing happens behind the GM's back.
    let findings: Vec<serde_json::Value> = db
        .query("SELECT payload FROM lint_finding WHERE kind = 'auto_alias'")
        .await.unwrap().take(0).unwrap();
    assert_eq!(findings.len(), 1, "an auto-alias must be listed for review");
}

#[tokio::test]
async fn an_ambiguous_link_refuses_to_guess_and_offers_candidates() {
    let db = seeded_db_two_quassars().await; // "The Quassar Family" AND "The Quassar Cartel"

    let matched = parse_and_sync_wikilinks(
        &db, "npc", "s", "Met [[The Quassars]] today.",
        WikilinkScope::Campaign { campaign_id: "c1" },
    ).await.expect("resolve");

    assert!(matched.is_empty(), "two candidates means it must NOT pick one");

    // No alias was written to either.
    let all: Vec<Vec<String>> = db
        .query("SELECT VALUE aliases FROM faction").await.unwrap().take(0).unwrap();
    assert!(all.iter().all(|a| a.is_empty()), "an ambiguous link must write no alias");
}
```

(Write `seeded_db()` / `seeded_db_two_quassars()` as local helpers following the `db()` pattern in `apps/desktop/src-tauri/tests/vault_inbound.rs`.)

- [ ] **Step 2: Run and watch it fail**

Run: `cargo test -p Chronacle --test wikilink_fuzzy_test`
Expected: FAIL — `matched` is empty; nothing resolves.

- [ ] **Step 3: Implement `add_alias` with collision validation**

`crates/chronacle-extraction/src/entity_service/aliases.rs`:

```rust
//! Alternate-name management. An alias must be unambiguous WITHIN ITS SCOPE, or
//! tier-2 resolution stops being deterministic and the same link resolves
//! differently depending on row order.

use crate::naming::normalize;
use crate::wikilink::{query_all_entity_names, WikilinkScope};
use super::EntityError;

/// Append an alias, refusing one that collides with another entity's name or
/// alias in the same resolution scope. The collision is a REFUSAL, not a
/// silent skip — the caller (and the GM) must know it did not take.
pub async fn add_alias<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    full_id: &str,
    alias: &str,
    scope: WikilinkScope<'_>,
) -> Result<(), EntityError> {
    let alias = alias.trim();
    if alias.is_empty() {
        return Err(EntityError::Validation {
            field: "alias".into(),
            message: "An alternate name cannot be empty".into(),
        });
    }

    let norm = normalize(alias);
    let entities = query_all_entity_names(db, &scope)
        .await
        .map_err(|e| EntityError::Database { message: e.to_string() })?;

    if let Some(other) = entities.iter().find(|e| {
        e.id != full_id
            && (normalize(&e.name) == norm || e.aliases.iter().any(|a| normalize(a) == norm))
    }) {
        return Err(EntityError::Validation {
            field: "alias".into(),
            message: format!("\"{alias}\" is already used by {}", other.name),
        });
    }

    let (table, id) = full_id.split_once(':').ok_or_else(|| EntityError::Validation {
        field: "id".into(),
        message: format!("Malformed record id: {full_id}"),
    })?;
    db.query(format!(
        "UPDATE type::thing('{table}', $id) SET aliases += $alias, updated_at = time::now()"
    ))
    .bind(("id", id.to_owned()))
    .bind(("alias", alias.to_owned()))
    .await
    .map_err(|e| EntityError::Database { message: e.to_string() })?
    .check()
    .map_err(|e| EntityError::Database { message: e.to_string() })?;
    Ok(())
}
```

`remove_alias` is the same shape with `aliases -= $alias` and no validation.

- [ ] **Step 4: Wire tier 4 into `parse_and_sync_wikilinks`**

In `mod.rs`, after `resolve_exact` returns `None` for a link:

```rust
        // Tier 4. Fuzzy — the only tier that can be WRONG, so it is the only
        // one that must be unambiguous, persisted, and reviewable.
        match naming::best_match(link_text, &names_only, naming::DEFAULT_THRESHOLD) {
            naming::MatchOutcome::Unique { id, score, .. } => {
                // Remember the decision: the next pass hits tier 2, so the
                // fuzzy path runs once per variant, ever — and the edge stays
                // explainable after the fact.
                aliases::add_alias(db, id, link_text, scope_for(&scope)).await.ok();
                codex_service::record_lint(
                    db,
                    "auto_alias",
                    json!({ "entity": id, "alias": link_text,
                            "similarity": score, "source": source_full_id }),
                ).await.ok();
                Some(id.to_string())
            }
            // Ambiguous or None: do NOT guess. Fall through to broken_wikilink,
            // carrying the ranked candidates that power "did you mean ...?".
            _ => None,
        }
```

`add_alias` failing (a collision) must not abort the pass — the link simply stays unresolved. Same for `record_lint`: a missing review row is a worse-than-ideal UX, not a data error.

- [ ] **Step 5: Attach candidates to `broken_wikilink`**

In `lint.rs::lint_broken_wikilinks`, replace the payload with one carrying ranked candidates:

```rust
                    let candidates = match naming::best_match(
                        link_text, &names_only, naming::DEFAULT_THRESHOLD * 0.8,
                    ) {
                        // A lower bar than auto-resolve on purpose: a SUGGESTION
                        // may be speculative because the GM adjudicates it.
                        naming::MatchOutcome::Unique { id, name, score } => {
                            vec![json!({ "id": id, "name": name, "similarity": score })]
                        }
                        naming::MatchOutcome::Ambiguous(cs) => cs
                            .iter()
                            .map(|c| json!({ "id": c.id, "name": c.name, "similarity": c.similarity }))
                            .collect(),
                        naming::MatchOutcome::None => vec![],
                    };
                    record_lint(
                        db,
                        "broken_wikilink",
                        json!({ "entity": full_id, "link_text": link_text,
                                "candidates": candidates }),
                    ).await?;
```

- [ ] **Step 6: Add the `alias_collision` detector**

New detector in `lint.rs`, called from `lint_pass`: group all in-scope entities by `normalize(name)` **and** every `normalize(alias)`; any normalized key claimed by two different records gets one `alias_collision` finding `{ alias, a, b }` (deduped via the existing `finding_exists_2`).

- [ ] **Step 7: Run the tests**

Run: `cargo test -p Chronacle --test wikilink_fuzzy_test && cargo test -p chronacle-extraction`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/chronacle-extraction apps/desktop/src-tauri/tests/wikilink_fuzzy_test.rs
git commit -m "feat(identity): fuzzy link auto-resolve with reviewable aliases"
```

---

### Task 5 (F5): Fuzzy duplicate detection

**Files:**

- Modify: `crates/chronacle-extraction/src/codex_service/lint.rs` (`lint_duplicates`)
- Test: `crates/chronacle-extraction/src/codex_service/lint_tests.rs`

**Interfaces:**

- Consumes: `naming::{normalize, similarity, DEFAULT_THRESHOLD}`.
- Produces: `duplicate_entity` findings with a **real** `similarity` (was hardcoded `1.0`).

- [ ] **Step 1: Write the failing test**

```rust
    #[tokio::test]
    async fn duplicate_detection_catches_a_leading_article_variant() {
        let db = db().await;
        seed_faction(&db, "f1", "The Free League").await;
        seed_faction(&db, "f2", "Free League").await;

        lint_pass(&db, "c1").await.expect("lint");

        // Today these hash to different keys and are NEVER reported.
        assert_eq!(kind_count(&db, "duplicate_entity").await, 1);
    }

    #[tokio::test]
    async fn duplicate_detection_does_not_flag_distinct_entities() {
        let db = db().await;
        seed_faction(&db, "f1", "The Legion").await;
        seed_location(&db, "l1", "The Legionnaire's Rest").await;
        seed_faction(&db, "f2", "Iron Host").await;

        lint_pass(&db, "c1").await.expect("lint");

        // A lint inbox full of non-duplicates is worse than none. Note the
        // tavern is a DIFFERENT TABLE from the faction and must never pair
        // with it regardless of score.
        assert_eq!(kind_count(&db, "duplicate_entity").await, 0);
    }
```

- [ ] **Step 2: Run and watch the first fail**

Run: `cargo test -p chronacle-extraction lint_tests`
Expected: FAIL — `duplicate_entity` count is 0, not 1.

- [ ] **Step 3: Rewrite `lint_duplicates` as two stages**

```rust
async fn lint_duplicates<C: Connection>(
    db: &surrealdb::Surreal<C>,
    entities: &[(String, String)],
) -> Result<usize, String> {
    // Stage 1: exact grouping on the NORMALIZED name. Catches
    // "The Free League" / "Free League" with no scoring at all — same table,
    // same normalized key, similarity 1.0.
    let mut groups: HashMap<(String, String), Vec<String>> = HashMap::new();
    for (full_id, name) in entities {
        let Some((table, _)) = split_full_id(full_id) else { continue };
        groups
            .entry((table.to_string(), crate::naming::normalize(name)))
            .or_default()
            .push(full_id.clone());
    }

    let mut new_findings = 0;
    for ids in groups.values() {
        for (a, b) in pairs(ids) {
            new_findings += record_duplicate(db, &a, &b, 1.0).await?;
        }
    }

    // Stage 2: fuzzy across the remaining pairs, WITHIN THE SAME TABLE only.
    // A faction and a tavern with similar names are not duplicates, whatever
    // the string says.
    let by_table = group_ids_by_table(entities);
    for (_table, members) in &by_table {
        for (i, (id_a, name_a)) in members.iter().enumerate() {
            for (id_b, name_b) in members.iter().skip(i + 1) {
                let na = crate::naming::normalize(name_a);
                let nb = crate::naming::normalize(name_b);
                if na == nb {
                    continue; // already reported by stage 1
                }
                let score = crate::naming::similarity(&na, &nb);
                if score >= crate::naming::DEFAULT_THRESHOLD {
                    new_findings += record_duplicate(db, id_a, id_b, score).await?;
                }
            }
        }
    }

    Ok(new_findings)
}

/// Writes one deduped `duplicate_entity` finding. Returns 1 if new, 0 if it
/// already existed.
async fn record_duplicate<C: Connection>(
    db: &surrealdb::Surreal<C>,
    a: &str,
    b: &str,
    similarity: f64,
) -> Result<usize, String> {
    let mut pair = [a.to_string(), b.to_string()];
    pair.sort();
    let [a, b] = pair;
    if finding_exists_2(db, "duplicate_entity", "a", &a, "b", &b).await? {
        return Ok(0);
    }
    record_lint(db, "duplicate_entity", json!({ "a": a, "b": b, "similarity": similarity })).await?;
    Ok(1)
}
```

(`group_ids_by_table` must be adapted to carry names; `pairs(ids)` is a small helper yielding each unordered pair once.)

- [ ] **Step 4: Run the tests**

Run: `cargo test -p chronacle-extraction lint`
Expected: PASS — including `lint_pass_is_idempotent_no_duplicate_findings` (dedup still holds).

- [ ] **Step 5: Commit**

```bash
git add crates/chronacle-extraction/src/codex_service
git commit -m "feat(identity): detect duplicates across name variants"
```

---

### Task 6 (F6): Rename-safe vault keys — the move decision

**Must land before merge.** Merge renames by definition, and today a rename strands the GM's edits.

**Files:**

- Modify: `crates/chronacle-vault/src/reconcile.rs`
- Modify: `crates/chronacle-vault/src/render.rs`, `frontmatter.rs` (the aliases seam)
- Modify: `crates/chronacle-core/src/vault.rs` (`EntityRecord.aliases`), `crates/chronacle-domain/src/vault_record_store.rs`
- Test: `apps/desktop/src-tauri/tests/vault_rename_test.rs`

**Interfaces:**

- Consumes: `SyncedRow { vref, key, synced_hash, conflict }` (already exists — it stores the **last-synced key**, which is what makes move detection possible).
- Produces: reconcile handles a changed key as a **move**; `ReconcileReport.moved: usize`.

- [ ] **Step 1: Write the failing test — the load-bearing one**

`apps/desktop/src-tauri/tests/vault_rename_test.rs`:

```rust
//! Renaming a record changes its vault key (keys derive from the name slug).
//! Reconcile must MOVE the file, not delete-and-re-export it.
//!
//! THE BUG THIS CATCHES: with delete+export, a file the GM edited outside the
//! app no longer matches its base, so the orphan sweep (correctly) refuses to
//! delete it — leaving a stale duplicate on disk whose edits never reach the
//! DB. Merge would hit this constantly.

#[tokio::test]
async fn renaming_a_record_moves_its_file_and_keeps_the_gms_edits() {
    let db = db().await;                       // seeds npc:n1 "Seraphina"
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());

    svc.reconcile().await.expect("initial export");
    let old = dir.path().join("campaigns/sov/entities/npc/seraphina.md");
    assert!(old.exists());

    // The GM edits the file in Obsidian — NOT yet synced back.
    let edited = std::fs::read_to_string(&old).unwrap()
        .replace("## Notes\n\n", "## Notes\n\nShe carries a silver key.\n");
    std::fs::write(&old, &edited).unwrap();

    // Meanwhile the record is renamed in-app.
    db.query("UPDATE npc:n1 SET name = 'Seraphina Aldric', updated_at = time::now()")
        .await.unwrap().check().unwrap();

    svc.reconcile().await.expect("reconcile after rename");

    let new = dir.path().join("campaigns/sov/entities/npc/seraphina-aldric.md");
    assert!(new.exists(), "the file must move to the new key");
    assert!(!old.exists(), "the old file must not linger as a stale duplicate");

    // And the GM's edit must have survived the rename — it is not collateral.
    let notes: Vec<Option<String>> = db
        .query("SELECT VALUE notes FROM npc:n1").await.unwrap().take(0).unwrap();
    assert!(
        notes[0].as_deref().unwrap_or_default().contains("silver key"),
        "a rename must never eat an unsynced GM edit"
    );
}
```

- [ ] **Step 2: Run and watch it fail**

Run: `cargo test -p Chronacle --test vault_rename_test`
Expected: FAIL — the old file still exists (orphan sweep refused to delete a diverged file) and "silver key" never reached the DB.

- [ ] **Step 3: Implement move detection in `reconcile()`**

Before computing the sync decision for a record, compare its **computed** key with the **stored** key from its `SyncedRow`:

```rust
        // A record whose computed key differs from its last-synced key has been
        // RENAMED (keys derive from the name slug). This is a move, not a death
        // and a birth: `decide()` sees only hashes and would read the absent new
        // key as a first export and the orphaned old key as a GM deletion.
        if let Some(state) = state_for(&vref) {
            if state.key != key {
                match self.move_one(&vref, &state, &key).await {
                    Ok(()) => { report.moved += 1; }
                    Err(e) => {
                        eprintln!("vault: move of {} failed: {e}", vref.to_thing());
                        report.failed += 1;
                    }
                }
                continue;
            }
        }
```

and `move_one`:

```rust
    /// Move a record's file from its last-synced key to its new one.
    ///
    /// Ordered so the GM's work is never collateral damage of a rename:
    /// an unsynced edit at the OLD key is applied inbound FIRST, then the file
    /// moves. Doing it the other way round would export over the edit.
    async fn move_one(
        &self,
        vref: &VaultRef,
        state: &SyncedRow,
        new_key: &str,
    ) -> Result<(), VaultError> {
        let old = self.store.read(&state.key).await.ok();

        if let Some(content) = &old {
            let hash = content_hash(content);
            if Some(hash) != state.synced_hash {
                // The GM edited this file and we have not applied it yet.
                // Apply BEFORE moving — otherwise the re-export at the new key
                // would silently overwrite their edit.
                self.apply_inbound(vref, &state.key, content).await?;
            }
            self.pending.arm_delete(&state.key);
            self.store.delete(&state.key).await?;
        }

        // Re-render (the record changed — that is why the key moved) and export
        // at the new key, which re-arms the write guard and sets the new base.
        self.export_one(vref, new_key).await
    }
```

- [ ] **Step 4: Fix the frontmatter aliases seam**

**The `aliases` frontmatter key already exists and means something else.** `render.rs:29` writes `aliases: vec![e.name.clone()]` — a _derived_ list whose only job is to make Obsidian resolve `[[Display Name]]` to a slug-named file. It is not GM data.

Export (`render.rs`) — union, name first:

```rust
                aliases: std::iter::once(e.name.clone())
                    .chain(e.aliases.iter().cloned())
                    .collect(),
```

Inbound (`frontmatter.rs` / wherever `GmParts` is parsed) — subtract the name, case-insensitively:

```rust
    /// The GM's alternate names are everything in the frontmatter `aliases`
    /// list EXCEPT the entity's own name, which we put there ourselves so
    /// Obsidian's `[[Display Name]]` links resolve to the slug-named file.
    /// Without this subtraction, every inbound sync would read the entity's own
    /// name back as a GM-authored alternate name.
    fn gm_aliases(frontmatter_aliases: &[String], name: &str) -> Vec<String> {
        frontmatter_aliases
            .iter()
            .filter(|a| !a.eq_ignore_ascii_case(name))
            .cloned()
            .collect()
    }
```

Add `aliases` to `EntityRecord` (core) and to the `SELECT`/`apply_gm_parts` statements in `vault_record_store.rs`.

- [ ] **Step 5: Add the round-trip test**

```rust
    #[test]
    fn the_alias_frontmatter_round_trip_is_idempotent() {
        // export -> parse -> export must be byte-identical, or every sync
        // would either grow the alias list or eat the GM's entries.
        let rendered = render_record(&entity_with_aliases(&["The Quassars"]));
        let parsed = parse(&rendered).expect("parse");
        assert_eq!(gm_aliases(&parsed.aliases, "The Quassar Family"), vec!["The Quassars"]);
        assert_eq!(render_record(&entity_with_aliases(&["The Quassars"])), rendered);
    }
```

- [ ] **Step 6: Run**

Run: `cargo test -p Chronacle --test vault_rename_test && cargo test -p chronacle-vault`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/chronacle-vault crates/chronacle-core crates/chronacle-domain apps/desktop/src-tauri/tests/vault_rename_test.rs
git commit -m "fix(vault): a rename must move the file, not lose the GM's edits"
```

---

### Task 7 (F7): The merge operation

**Files:**

- Create: `crates/chronacle-extraction/src/entity_service/merge.rs`
- Modify: `apps/desktop/src-tauri/src/commands/entity_commands.rs`
- Test: `apps/desktop/src-tauri/tests/entity_merge_test.rs`

**Interfaces:**

- Consumes: `aliases::add_alias` (Task 4); rename-safe reconcile (Task 6); `soft_delete` (existing).
- Produces:
  - `pub struct MergeChoices { pub summary: FieldChoice, pub notes: FieldChoice }`
  - `pub enum FieldChoice { KeepSurvivor, KeepLoser, KeepBoth }`
  - `pub async fn merge(db, survivor: &str, loser: &str, choices: MergeChoices) -> Result<(), EntityError>`
  - Tauri command `merge_entities(survivorId, loserId, choices)`

- [ ] **Step 1: Write the failing test**

```rust
//! Merge folds two records into one WITHOUT losing anything cheap to keep.

#[tokio::test]
async fn merge_unions_every_edge_and_keeps_the_losers_name_as_an_alias() {
    let db = seeded().await;
    // faction:a "The Free League"  --allied_with--> npc:x
    // faction:b "Free League"      --enemy_of-->    npc:y

    entity_service::merge(&db, "faction:a", "faction:b", MergeChoices {
        summary: FieldChoice::KeepSurvivor,
        notes: FieldChoice::KeepBoth,
    }).await.expect("merge");

    // NO EDGE IS EVER DROPPED. A relationship is a fact about the world, not a
    // stylistic preference — the survivor must know everything both knew.
    let related = entity_service::list_related(&db, "a", EntityKind::Faction).await.unwrap();
    let names: Vec<&str> = related.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"X"), "the survivor's own edge must survive");
    assert!(names.contains(&"Y"), "the loser's edge must be re-pointed, not dropped");

    // Every [[Free League]] link ever written must keep working.
    let aliases: Vec<Vec<String>> = db.query("SELECT VALUE aliases FROM faction:a")
        .await.unwrap().take(0).unwrap();
    assert!(aliases[0].iter().any(|a| a == "Free League"));

    // KeepBoth concatenated, nothing silently destroyed.
    let notes: Vec<Option<String>> = db.query("SELECT VALUE notes FROM faction:a")
        .await.unwrap().take(0).unwrap();
    let notes = notes[0].clone().unwrap();
    assert!(notes.contains("Merged from Free League"));

    // The article was compiled from half the facts; it must be recompiled.
    let stale: Vec<bool> = db.query("SELECT VALUE codex_stale FROM faction:a")
        .await.unwrap().take(0).unwrap();
    assert!(stale[0]);

    // The loser is soft-deleted, never hard-deleted.
    let deleted: Vec<bool> = db.query("SELECT VALUE vault_deleted FROM faction:b")
        .await.unwrap().take(0).unwrap();
    assert!(deleted[0]);
}
```

- [ ] **Step 2: Run and watch it fail**

Run: `cargo test -p Chronacle --test entity_merge_test`
Expected: FAIL — `merge` does not exist.

- [ ] **Step 3: Implement `merge`**

```rust
/// Fold `loser` into `survivor`.
///
/// CRASH SAFETY. There is no transaction here: the codebase uses none, and
/// merge also does non-DB work (an embedding call, a vault file removal) that
/// no DB transaction could cover. So the ORDER is the safety property —
/// edges first, soft-delete LAST. Every step before the delete is idempotent
/// and re-runnable, so a crash mid-merge leaves both records alive with a
/// SUPERSET of edges: visibly unfinished, safe, and re-runnable. Deleting
/// first would orphan edges permanently.
pub async fn merge<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    survivor: &str,
    loser: &str,
    choices: MergeChoices,
) -> Result<(), EntityError> {
    if survivor == loser {
        return Err(EntityError::Validation {
            field: "loser".into(),
            message: "Cannot merge a record into itself".into(),
        });
    }
    let (s, l) = (load(db, survivor).await?, load(db, loser).await?);

    // 1. Re-point every edge onto the survivor, both directions, deduped.
    //    RELATE is idempotent here because we skip pairs that already exist.
    repoint_edges(db, loser, survivor).await?;

    // 2. Aliases: union, plus the loser's NAME — this is what keeps every
    //    [[Free League]] the GM ever wrote working after the merge.
    let mut aliases = s.aliases.clone();
    for a in l.aliases.iter().chain(std::iter::once(&l.name)) {
        if !aliases.iter().any(|x| x.eq_ignore_ascii_case(a)) {
            aliases.push(a.clone());
        }
    }

    // 3. Field choices. KeepBoth concatenates; nothing is silently destroyed.
    let summary = choose(&choices.summary, s.summary.as_deref(), l.summary.as_deref(), &l.name);
    let notes = choose(&choices.notes, s.notes.as_deref(), l.notes.as_deref(), &l.name);

    // 4. Write the survivor, marking the article stale: it was compiled from
    //    half the facts. Merging two articles textually would produce prose no
    //    compiler wrote and no citation supports.
    let (table, id) = split(survivor)?;
    db.query(format!(
        "UPDATE type::thing('{table}', $id) SET aliases = $aliases, summary = $summary, \
         notes = $notes, codex_stale = true, updated_at = time::now()"
    ))
    .bind(("id", id.to_owned()))
    .bind(("aliases", aliases))
    .bind(("summary", opt_value(summary)))
    .bind(("notes", opt_value(notes)))
    .await
    .map_err(db_err)?
    .check()
    .map_err(db_err)?;

    // 5. Soft-delete the loser LAST. Its vault file goes through the normal
    //    reconcile sweep — never a raw DELETE.
    soft_delete(db, id_of(loser), kind_of(loser)?).await?;

    Ok(())
}

fn choose(c: &FieldChoice, s: Option<&str>, l: Option<&str>, loser_name: &str) -> Option<String> {
    match c {
        FieldChoice::KeepSurvivor => s.map(str::to_string),
        FieldChoice::KeepLoser => l.map(str::to_string),
        FieldChoice::KeepBoth => match (s, l) {
            (Some(s), Some(l)) => Some(format!("{s}\n\n## Merged from {loser_name}\n\n{l}")),
            (Some(s), None) => Some(s.to_string()),
            (None, Some(l)) => Some(l.to_string()),
            (None, None) => None,
        },
    }
}
```

Re-embed the survivor after the merge (its name/summary/notes changed) via the existing `embed_node`, and resolve the `duplicate_entity` finding for the pair.

- [ ] **Step 4: Add the Tauri command**

In `entity_commands.rs` — `merge_entities(state, survivor_id, loser_id, choices)`, `#[serde(rename_all = "camelCase")]` on the choices DTO (the frontend sends camelCase). Register it in `lib.rs`'s `invoke_handler`.

- [ ] **Step 5: Run**

Run: `cargo test -p Chronacle --test entity_merge_test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/chronacle-extraction/src/entity_service apps/desktop/src-tauri
git commit -m "feat(identity): merge two entities without losing edges"
```

---

### Task 8 (F8): Campaign rename

**Files:**

- Modify: `crates/chronacle-domain/src/campaign_service.rs`
- Modify: `apps/desktop/src-tauri/src/commands/campaign_commands.rs`
- Test: `apps/desktop/src-tauri/tests/campaign_rename_test.rs`

**Interfaces:**

- Consumes: the move decision from Task 6 (this is the same machinery one level up — every key under `campaigns/<slug>/` moves at once).
- Produces: `campaign_service::rename(db, id, new_name)`; Tauri command `rename_campaign`.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn renaming_a_campaign_moves_every_file_beneath_it() {
    let db = db().await;                    // campaign:c1 "SoV", npc:n1 "Seraphina"
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());
    svc.reconcile().await.expect("export");
    assert!(dir.path().join("campaigns/sov/entities/npc/seraphina.md").exists());

    campaign_service::rename(&db, "c1", "Shadows over Valheim").await.expect("rename");
    svc.reconcile().await.expect("reconcile");

    assert!(dir.path().join("campaigns/shadows-over-valheim/entities/npc/seraphina.md").exists());
    assert!(!dir.path().join("campaigns/sov").exists(), "the old folder must not linger");
}
```

- [ ] **Step 2: Run and watch it fail**

Run: `cargo test -p Chronacle --test campaign_rename_test`
Expected: FAIL — `rename` does not exist.

- [ ] **Step 3: Implement**

`rename` is a plain `UPDATE campaign SET name = $name`. The vault side needs **no new code** — every record under the campaign now computes a different key, and Task 6's move decision relocates each one, carrying its base and applying any unsynced GM edit first. Confirm the now-empty `campaigns/<old-slug>/` directory is cleaned by the orphan sweep; if it is not, prune empty managed directories at the end of the sweep.

- [ ] **Step 4: Run**

Run: `cargo test -p Chronacle --test campaign_rename_test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/chronacle-domain apps/desktop/src-tauri
git commit -m "feat(vault): rename a campaign and move its vault folder"
```

---

### Task 9 (F9): Frontend — alternate names, did-you-mean, merge dialog, auto-linked

**GM-facing term is "alternate names".** Never "aliases" in any string the GM reads.

**Files:**

- Create: `apps/desktop/src/components/AliasField.svelte`, `apps/desktop/src/components/MergeDialog.svelte`
- Modify: `apps/desktop/src/views/MaintenanceView.svelte`, the entity detail view, campaign settings
- Modify: `apps/desktop/src/lib/` invoke wrappers
- Test: `*.test.ts` (Vitest + `@testing-library/svelte`); `apps/desktop/tests/e2e/features/entity-identity.feature`

**Interfaces:**

- Consumes: `merge_entities`, `set_entity_aliases`, `confirm_alias_suggestion`, `undo_auto_alias`, `rename_campaign`.

- [ ] **Step 1: Write the failing component test**

```ts
it("shows the suggestion and confirms it as an alternate name", async () => {
  render(MaintenanceView, {
    props: { findings: [brokenWikilinkWithCandidate] },
  });

  expect(screen.getByText(/did you mean/i)).toBeInTheDocument();
  expect(screen.getByText("The Quassar Family")).toBeInTheDocument();

  await fireEvent.click(screen.getByRole("button", { name: /yes/i }));

  expect(invoke).toHaveBeenCalledWith("confirm_alias_suggestion", {
    entityId: "faction:q",
    alias: "The Quassars",
  });
});

it('never says the word "aliases" to the GM', () => {
  render(AliasField, { props: { aliases: ["The Quassars"] } });
  expect(screen.queryByText(/alias/i)).not.toBeInTheDocument();
  expect(screen.getByText(/alternate names/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run and watch them fail**

Run: `pnpm -C apps/desktop test:run`
Expected: FAIL — components do not exist.

- [ ] **Step 3: Build the components (Svelte 5 runes only)**

- `AliasField.svelte` — chips with add/remove. Hint: _"Alternate names this is known by. Links using any of them will find this entity."_
- `MergeDialog.svelte` — side by side; survivor radio; per-field `keep A / keep B / keep both`; a plain-language consequence line: _"12 relationships merged, 3 alternate names kept, the codex article will be rewritten."_
- `MaintenanceView.svelte` —
  - `broken_wikilink` with candidates → _"[[The Quassars]] — did you mean **The Quassar Family**?"_ + confirm;
  - `duplicate_entity` → **Merge** button opening the dialog;
  - `auto_alias` → a collapsed _Auto-linked_ list with **Undo**, framed as reviewable-not-required;
  - `alias_collision` → both claimants linked.

- [ ] **Step 4: Add the acceptance scenarios (ADR-011)**

`apps/desktop/tests/e2e/features/entity-identity.feature`:

```gherkin
Feature: Alternate names and duplicates

  Scenario: Confirming a suggested alternate name fixes the link
    Given an entity "The Quassar Family" exists
    And a note links to "[[The Quassars]]" and the link is unresolved
    When the GM confirms the suggestion "The Quassar Family"
    Then "The Quassars" is listed among that entity's alternate names
    And the broken link is resolved

  Scenario: Merging two entities keeps every relationship
    Given duplicate factions "The Free League" and "Free League"
    And each has a relationship the other does not
    When the GM merges them keeping "The Free League"
    Then the surviving faction has both relationships
    And "Free League" is one of its alternate names
```

**Every scenario must be mutation-checked** — delete the feature code and confirm the scenario fails. A backend-E2E step that only calls `page.goto` proves nothing; in tranche 5, five of six scenarios would have passed with the feature deleted.

- [ ] **Step 5: Run**

Run: `pnpm -C apps/desktop test:run && pnpm -C apps/desktop exec playwright test tests/e2e/backend/`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src apps/desktop/tests
git commit -m "feat(identity): alternate names, merge dialog, did-you-mean"
```

---

### Task 10 (F10): Threshold tuning on real data, docs, ADR

The threshold is a **deliverable**, not a constant pulled from the air.

**Files:**

- Create: `apps/desktop/src-tauri/src/commands/identity_dryrun.rs` (dev-only, read-only)
- Modify: `docs/user-guide.md`, `docs/architecture.md`

- [ ] **Step 1: Build the read-only dry-run**

A command that runs fuzzy duplicate detection and fuzzy link resolution over the **real** database and **reports what it would do** — writing nothing. Output: every proposed auto-alias and duplicate pair with its score, sorted by score.

- [ ] **Step 2: Run it against the maintainer's real campaign**

Back up `chronacle.db` first. Show the maintainer the ranked list and agree the threshold **against real names**, not fixtures. A green suite is not evidence a threshold is right; fresh fixtures cannot tell us what a real world looks like.

- [ ] **Step 3: Fix `DEFAULT_THRESHOLD` and record the evidence in ADR-012**

Update the constant in `naming.rs`. ADR-012 records the chosen value **and the data behind it** — the false positives it excludes and the true positives it keeps.

- [ ] **Step 4: Ship the user guide**

Add the **"Names and duplicates"** chapter to `docs/user-guide.md`. **The copy is already drafted in the spec** (`## Documentation plan`) — transfer it verbatim; do not reinvent it. Also add the campaign-rename paragraph to "Managing Campaigns" and the alternate-names paragraph to "Your Vault".

- [ ] **Step 5: Write ADR-012 in `docs/architecture.md`**

Cover: aliases as the identity primitive; the four tiers; why fuzzy auto-resolve persists and surfaces its decisions; why merge is ordered edges-first-delete-last; rename as a move. Tick the Phase-3 checklist items this tranche closes (vault sync; campaign rename), and correct the vault-sync line, which still describes a timestamped `.conflict.<ts>.md` sidecar that never shipped.

- [ ] **Step 6: Run the full CI gate**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny check
pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint
pnpm -C apps/desktop test:run
pnpm -C apps/desktop exec playwright test tests/e2e/backend/
```

All must pass — **including `cargo deny check`**, which is the one that gets skipped and the one that has caught a real advisory.

- [ ] **Step 7: Run the real app on the real vault**

Launch the built app against the maintainer's campaign and an Obsidian vault. Rename an entity, merge a duplicate, write an elided link. Tranche 5's two worst bugs were invisible to a 100%-green suite and surfaced within minutes of doing exactly this.

- [ ] **Step 8: Commit and open the PR**

```bash
git add docs crates apps
git commit -m "docs: ADR-012 entity identity, user guide, tuned threshold"
```

---

## Self-Review

**Spec coverage:** aliases schema + backfill → T1. Normalization/similarity → T2. Tiers 1–3 → T3. Tier 4 + auto_alias + candidates + alias_collision → T4. Fuzzy duplicates → T5. Rename safety + frontmatter seam → T6. Merge → T7. Campaign rename → T8. UX + UI hints + `.feature` → T9. Threshold tuning + user guide + ADR → T10. Every spec section maps to a task.

**Type consistency:** `EntityIdentity { id, name, aliases }` is produced in T3 and consumed by T4's `add_alias` and `best_match`. `MatchOutcome`/`Candidate`/`DEFAULT_THRESHOLD` are defined in T2 and used in T4 and T5. `MergeChoices`/`FieldChoice` are defined in T7 and consumed by T9's dialog. `SyncedRow.key` (existing) is what T6's move detection reads.

**Open item carried from the spec:** whether `rule_entry` joins fuzzy _duplicate_ detection (T5 covers entity tables only; rules get aliases via T1 but not fuzzy dedup). Proposed and unchanged: aliases yes, fuzzy dedup deferred — a falsely merged rule is worse than a duplicated one.
