# Defined Entity Kinds + Canonical Relationship Vocabulary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make LLM extraction classify families/groups as `faction` and draw `rel_type` from a known, canonical, direction-normalized vocabulary — with no relations dropped and no database migration.

**Architecture:** Two prompt edits add per-kind definitions and an enumerated relationship vocabulary. A new `RelType` enum (known variants + `Other(String)` catch-all) parses LLM output and normalizes inverse phrasings to a canonical direction by flipping the stored edge. Normalization happens at the `persist_batch` boundary; the `rel_type` DB column stays a string, so "unknown" is derived, not stored.

**Tech Stack:** Rust, SurrealDB (in-memory `mem::Db` for tests), `tokio::test`, `mockall`-style hand-rolled mocks already present in `extraction_service.rs`.

---

## File Structure

- `src-tauri/src/services/entity_service.rs` — add the `RelType` enum + its tests, alongside the existing `EntityKind`. This is the home of entity/relationship domain types.
- `src-tauri/src/services/extraction_service.rs` — add two shared prompt-fragment consts (`ENTITY_KIND_DEFS`, `REL_TYPE_VOCAB`), interpolate them into `build_extraction_prompt` and `build_seed_prompt`, and normalize `rel_type` in `persist_batch`. Add prompt + normalization tests in its existing `#[cfg(test)] mod tests`.

No schema files change. No frontend changes.

---

## Task 1: `RelType` enum

**Files:**
- Modify: `src-tauri/src/services/entity_service.rs` (add enum after `EntityKind`'s `impl` block ending at line 72; add unit tests in the existing `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `entity_service.rs` (near the existing `entity_kind_from_table_*` tests):

```rust
#[test]
fn rel_type_known_variants_roundtrip() {
    for key in [
        "leads", "member_of", "located_in", "owns", "serves", "created", "parent_of",
        "led_by", "has_member", "contains", "owned_by", "served_by", "created_by", "child_of",
        "allied_with", "enemy_of", "related_to", "knows",
    ] {
        let rt = RelType::from_llm(key);
        assert_eq!(rt.as_str(), key, "{key} must round-trip");
        assert!(rt.is_known(), "{key} must be known");
    }
}

#[test]
fn rel_type_unknown_becomes_other_and_is_preserved() {
    let rt = RelType::from_llm("secretly_betrays");
    assert_eq!(rt, RelType::Other("secretly_betrays".to_string()));
    assert_eq!(rt.as_str(), "secretly_betrays");
    assert!(!rt.is_known());
    // Unknown is never flipped.
    let (canon, flip) = rt.canonical();
    assert_eq!(canon, RelType::Other("secretly_betrays".to_string()));
    assert!(!flip);
}

#[test]
fn rel_type_inverse_normalizes_to_canonical_with_flip() {
    let cases = [
        ("led_by", "leads"),
        ("has_member", "member_of"),
        ("contains", "located_in"),
        ("owned_by", "owns"),
        ("served_by", "serves"),
        ("created_by", "created"),
        ("child_of", "parent_of"),
    ];
    for (inverse, canonical) in cases {
        let (canon, flip) = RelType::from_llm(inverse).canonical();
        assert_eq!(canon.as_str(), canonical, "{inverse} -> {canonical}");
        assert!(flip, "{inverse} must flip");
    }
}

#[test]
fn rel_type_canonical_and_symmetric_do_not_flip() {
    for key in ["leads", "member_of", "allied_with", "enemy_of", "related_to", "knows"] {
        let (canon, flip) = RelType::from_llm(key).canonical();
        assert_eq!(canon.as_str(), key);
        assert!(!flip, "{key} must not flip");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test --lib rel_type`
Expected: FAIL to compile — `cannot find type RelType in this scope`.

- [ ] **Step 3: Write the `RelType` enum**

Insert immediately after the `impl EntityKind { ... }` block (after line 72) in `entity_service.rs`:

```rust
// ── Relationship type ──────────────────────────────────────────────────────────

/// Canonical, finite vocabulary for `relates_to.rel_type`.
///
/// Both directions of each directional relationship are first-class variants so
/// the LLM always has a fitting type for the direction the source text describes
/// (no dropout). Inverse members normalize to their canonical counterpart via
/// [`RelType::canonical`], which also reports whether the edge must be flipped.
/// `Other` carries any unrecognised value verbatim — "unknown" is derived, not
/// stored, so no migration is needed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelType {
    // Directional — canonical direction.
    Leads,
    MemberOf,
    LocatedIn,
    Owns,
    Serves,
    Created,
    ParentOf,
    // Directional — inverse direction (normalize via `canonical()`).
    LedBy,
    HasMember,
    Contains,
    OwnedBy,
    ServedBy,
    CreatedBy,
    ChildOf,
    // Symmetric — direction irrelevant, self-inverse.
    AlliedWith,
    EnemyOf,
    RelatedTo,
    Knows,
    // Catch-all for unrecognised LLM output (stored verbatim).
    Other(String),
}

impl RelType {
    /// Parse a raw `rel_type` string from the LLM. Infallible: unknown values
    /// become `Other(raw)`. (Named `from_llm`, not `from_str`, to avoid clippy's
    /// `should_implement_trait` lint on an infallible parser.)
    pub fn from_llm(raw: &str) -> Self {
        match raw {
            "leads" => Self::Leads,
            "member_of" => Self::MemberOf,
            "located_in" => Self::LocatedIn,
            "owns" => Self::Owns,
            "serves" => Self::Serves,
            "created" => Self::Created,
            "parent_of" => Self::ParentOf,
            "led_by" => Self::LedBy,
            "has_member" => Self::HasMember,
            "contains" => Self::Contains,
            "owned_by" => Self::OwnedBy,
            "served_by" => Self::ServedBy,
            "created_by" => Self::CreatedBy,
            "child_of" => Self::ChildOf,
            "allied_with" => Self::AlliedWith,
            "enemy_of" => Self::EnemyOf,
            "related_to" => Self::RelatedTo,
            "knows" => Self::Knows,
            other => Self::Other(other.to_string()),
        }
    }

    /// Stable snake_case key for known variants; the raw string for `Other`.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Leads => "leads",
            Self::MemberOf => "member_of",
            Self::LocatedIn => "located_in",
            Self::Owns => "owns",
            Self::Serves => "serves",
            Self::Created => "created",
            Self::ParentOf => "parent_of",
            Self::LedBy => "led_by",
            Self::HasMember => "has_member",
            Self::Contains => "contains",
            Self::OwnedBy => "owned_by",
            Self::ServedBy => "served_by",
            Self::CreatedBy => "created_by",
            Self::ChildOf => "child_of",
            Self::AlliedWith => "allied_with",
            Self::EnemyOf => "enemy_of",
            Self::RelatedTo => "related_to",
            Self::Knows => "knows",
            Self::Other(s) => s.as_str(),
        }
    }

    /// True for any known variant; false only for `Other`.
    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Other(_))
    }

    /// Normalize to canonical direction. Returns `(canonical_variant, flip)`:
    /// when `flip` is true the caller must swap the edge's `in`/`out` endpoints.
    /// Canonical and symmetric variants (and `Other`) return `(self, false)`.
    pub fn canonical(&self) -> (RelType, bool) {
        match self {
            Self::LedBy => (Self::Leads, true),
            Self::HasMember => (Self::MemberOf, true),
            Self::Contains => (Self::LocatedIn, true),
            Self::OwnedBy => (Self::Owns, true),
            Self::ServedBy => (Self::Serves, true),
            Self::CreatedBy => (Self::Created, true),
            Self::ChildOf => (Self::ParentOf, true),
            other => (other.clone(), false),
        }
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test --lib rel_type`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/entity_service.rs
git commit -m "feat(entity): add RelType canonical relationship vocabulary

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: Normalize `rel_type` in `persist_batch`

**Files:**
- Modify: `src-tauri/src/services/extraction_service.rs` (import `RelType`; replace the `entity_service::relate` call in `persist_batch`, currently lines 408–424; add an integration test in the existing test module)

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `extraction_service.rs` (after `extract_creates_entities_with_collection_edge`):

```rust
#[tokio::test]
async fn extract_normalizes_inverse_rel_type_and_preserves_unknown() {
    let (db, col_id) = setup_db_with_collection().await;

    // Varn (npc) "led_by" Iron Fist (faction)  -> canonical: Iron Fist leads Varn (flipped)
    // Varn (npc) "betrays" Dark Pact (faction)  -> unknown: stored verbatim, not flipped
    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: r#"{
            "entities": [{
                "name": "Commander Varn",
                "kind": "npc",
                "summary": "Leader.",
                "notes": null,
                "relations": [
                    {"name": "The Iron Fist", "kind": "faction", "rel_type": "led_by", "summary": "Militia.", "notes": null},
                    {"name": "The Dark Pact", "kind": "faction", "rel_type": "betrays", "summary": "A pact.", "notes": null}
                ]
            }]
        }"#
        .to_string(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));

    extract_from_collection(&db, &llm, &embed, &col_id, |_| {})
        .await
        .unwrap();

    #[derive(serde::Deserialize)]
    struct Edge {
        #[serde(rename = "in")]
        in_thing: surrealdb::sql::Thing,
        #[serde(rename = "out")]
        out_thing: surrealdb::sql::Thing,
        rel_type: String,
    }
    let mut resp = db
        .query("SELECT in, out, rel_type FROM relates_to")
        .await
        .unwrap();
    let edges: Vec<Edge> = resp.take(0).unwrap();

    let leads = edges
        .iter()
        .find(|e| e.rel_type == "leads")
        .expect("inverse 'led_by' must normalize to canonical 'leads'");
    assert_eq!(leads.in_thing.tb, "faction", "edge must be flipped: faction is 'in'");
    assert_eq!(leads.out_thing.tb, "npc", "edge must be flipped: npc is 'out'");

    let betrays = edges
        .iter()
        .find(|e| e.rel_type == "betrays")
        .expect("unknown 'betrays' must be stored verbatim");
    assert_eq!(betrays.in_thing.tb, "npc", "unknown edge keeps original direction");
    assert_eq!(betrays.out_thing.tb, "faction");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd src-tauri && cargo test --lib extract_normalizes_inverse_rel_type`
Expected: FAIL — no edge with `rel_type == "leads"` exists yet (it is stored as `"led_by"`, unflipped), so the `.expect("...normalize to canonical 'leads'")` panics.

- [ ] **Step 3: Add the import**

In `extraction_service.rs`, change the `entity_service` import (currently line 16) to include `RelType`:

```rust
use crate::services::entity_service::{self, EntityInput, EntityKind, GraphNode, RelType};
```

- [ ] **Step 4: Replace the relate call with normalized-direction logic**

In `persist_batch`, replace this block (currently lines 408–424):

```rust
            let result = entity_service::relate(
                db,
                &origin_node.id,
                &origin_node.kind,
                &rel_node.id,
                &rel_node.kind,
                &rel.rel_type,
                None,
            )
            .await;
            match result {
                Ok(_) => relations_created += 1,
                Err(e) => eprintln!(
                    "extraction: failed to relate {} -> {} ({}): {e}",
                    origin_node.name, rel_node.name, rel.rel_type
                ),
            }
```

with:

```rust
            // Normalize to canonical direction: inverse rel_types (e.g. "led_by")
            // flip the edge so storage holds only canonical keys; "Other" values
            // are stored verbatim, unflipped.
            let (canonical, flip) = RelType::from_llm(&rel.rel_type).canonical();
            let (from_id, from_kind, to_id, to_kind) = if flip {
                (&rel_node.id, &rel_node.kind, &origin_node.id, &origin_node.kind)
            } else {
                (&origin_node.id, &origin_node.kind, &rel_node.id, &rel_node.kind)
            };
            let result = entity_service::relate(
                db,
                from_id,
                from_kind,
                to_id,
                to_kind,
                canonical.as_str(),
                None,
            )
            .await;
            match result {
                Ok(_) => relations_created += 1,
                Err(e) => eprintln!(
                    "extraction: failed to relate {} -> {} ({}): {e}",
                    origin_node.name, rel_node.name, canonical.as_str()
                ),
            }
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cd src-tauri && cargo test --lib extract_normalizes_inverse_rel_type`
Expected: PASS.

- [ ] **Step 6: Run the full extraction test module to check no regressions**

Run: `cd src-tauri && cargo test --lib extraction_service`
Expected: PASS (all existing extraction tests still green — the existing `rel_type: "commands"` cases now store as `Other("commands")` verbatim, which does not change their assertions).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/services/extraction_service.rs
git commit -m "feat(extraction): normalize rel_type to canonical direction on persist

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: Define entity kinds + relationship vocabulary in the prompts

**Files:**
- Modify: `src-tauri/src/services/extraction_service.rs` (add two consts; edit `build_extraction_prompt` (lines 128–166) and `build_seed_prompt` (lines 170–203); add prompt assertions in the test module)

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `extraction_service.rs` (near the existing `build_extraction_prompt_*` / `build_seed_prompt_*` tests):

```rust
#[test]
fn extraction_prompt_defines_kinds_and_rel_vocab() {
    let prompt = build_extraction_prompt("any text");
    // Faction definition routes families/houses correctly.
    assert!(prompt.contains("faction:"));
    assert!(prompt.contains("noble house, family, or clan"));
    // Other kind definitions present.
    assert!(prompt.contains("creature:"));
    assert!(prompt.contains("player_character:"));
    // Relationship vocabulary present, both directions.
    assert!(prompt.contains("leads / led_by"));
    assert!(prompt.contains("parent_of / child_of"));
    assert!(prompt.contains("allied_with"));
}

#[test]
fn seed_prompt_defines_kinds_and_rel_vocab() {
    let prompt = build_seed_prompt("Varn", "Varn leads the Iron Fist.");
    assert!(prompt.contains("noble house, family, or clan"));
    assert!(prompt.contains("leads / led_by"));
    assert!(prompt.contains("parent_of / child_of"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test --lib _defines_kinds_and_rel_vocab`
Expected: FAIL — assertions on `"noble house, family, or clan"` / `"leads / led_by"` fail (the current prompts contain neither).

- [ ] **Step 3: Add the shared prompt-fragment consts**

In `extraction_service.rs`, add immediately before `fn build_extraction_prompt` (before line 128):

```rust
/// Per-kind definitions shared by the classifying prompts. Keeps the two prompts
/// in sync (DRY) and is the fix for groups/families being mis-classified as npc.
const ENTITY_KIND_DEFS: &str = "Entity kinds (choose the single best fit):
- npc: a single named individual (person, named monster, deity).
- location: a place — region, settlement, building, room, or plane.
- faction: any organized group of people — guild, cult, government, military order, crime ring, noble house, family, or clan.
- creature: a kind or species of being, not a named individual (e.g. \"goblin\", \"owlbear\").
- item: an object, artifact, weapon, or substance.
- event: something that happens at a point or span in time.
- player_character: a player character (PC) controlled by a player.
- misc: anything that fits none of the above.";

/// Canonical relationship vocabulary shared by the classifying prompts. Mirrors
/// the `RelType` variants so the LLM emits values that normalize cleanly.
const REL_TYPE_VOCAB: &str = "Relationship types (choose the closest, in whichever direction matches the sentence; only if none fits, use a short snake_case verb):
- leads / led_by: X leads or commands group Y (led_by is the inverse).
- member_of / has_member: X belongs to group or family Y.
- located_in / contains: X is situated within place Y.
- owns / owned_by: X owns or possesses Y.
- serves / served_by: X serves or is loyal to Y.
- created / created_by: X created or founded Y.
- parent_of / child_of: X is a parent or ancestor of Y.
- allied_with: X and Y are allied (no direction).
- enemy_of: X and Y are enemies (no direction).
- related_to: X and Y are kin or otherwise associated (no direction).
- knows: X and Y are acquainted (no direction).";
```

- [ ] **Step 4: Edit `build_extraction_prompt`**

Replace the line (currently line 134):

```rust
- Identify its kind (one of: npc, location, faction, creature, item, event, player_character, misc)
```

with:

```rust
- Identify its kind using these definitions:
{ENTITY_KIND_DEFS}
```

And replace the `rel_type` schema example line (currently line 154):

```rust
          "rel_type": "string (e.g. leads, commands, located_in, allied_with)",
```

with:

```rust
          "rel_type": "one of the relationship types listed below",
```

And insert the vocabulary block just before `Return ONLY valid JSON` (currently line 141). The relevant region becomes:

```rust
- "notes": a more thorough description, including how this entity relates to others (its role, ties, and the connection to the entity it was extracted alongside). May contain [[wikilinks]]. Leave empty if there is nothing beyond the summary.

{REL_TYPE_VOCAB}

Return ONLY valid JSON matching this exact schema (no markdown, no explanation):
```

- [ ] **Step 5: Edit `build_seed_prompt`**

In `build_seed_prompt`, the entities are still described with the bare `"kind": "npc|location|..."` schema hint; add the definitions and vocabulary the same way. Replace the line (currently line 176):

```rust
- Output "{name}" as a single level-0 entity with its kind, summary, and notes.
```

with:

```rust
- Output "{name}" as a single level-0 entity with its kind, summary, and notes.
- Classify every entity's kind using these definitions:
{ENTITY_KIND_DEFS}
```

And insert the vocabulary block just before `Return ONLY valid JSON` (currently line 184). The relevant region becomes:

```rust
- "notes": a more thorough description, including how the entity relates to "{name}" (its role, ties, and connection). May contain [[wikilinks]]. Leave empty if there is nothing beyond the summary.

{REL_TYPE_VOCAB}

Return ONLY valid JSON matching this exact schema (no markdown, no explanation):
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test --lib _defines_kinds_and_rel_vocab`
Expected: PASS (2 tests). The pre-existing prompt tests (`build_extraction_prompt_contains_chunk_text`, `build_seed_prompt_anchors_on_entity_name`) must also still pass.

- [ ] **Step 7: Run the full extraction module + clippy**

Run: `cd src-tauri && cargo test --lib extraction_service && cargo clippy --all-targets --all-features -- -D warnings`
Expected: tests PASS, clippy clean (no `should_implement_trait` warning — the parser is named `from_llm`).

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/services/extraction_service.rs
git commit -m "feat(extraction): define entity kinds and rel_type vocabulary in prompts

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Final verification

- [ ] **Run the whole backend suite + format check:**

Run: `cd src-tauri && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test`
Expected: all green.

---

## Spec coverage check

- **Part A — kind definitions in both classifying prompts, faction covers families:** Task 3 (`ENTITY_KIND_DEFS`, both prompt edits, `"noble house, family, or clan"` assertion). ✓
- **Part B — `RelType` enum with `Other`, `from_llm`/`as_str`/`is_known`/`canonical`:** Task 1. ✓ (Spec named the derived check `is_canonical`; renamed to `is_known` to avoid confusion with `canonical()` — same behavior: false only for `Other`.)
- **Both directions exposed, normalized to canonical on store (flip):** Task 2 (`persist_batch` flip) + Task 3 (`REL_TYPE_VOCAB` lists both directions). ✓
- **No migration / column stays string / unknown derived:** No schema file touched; `Other` stored verbatim. ✓
- **No required frontend change:** none in plan. ✓
- **Testing — prompt assertions, RelType round-trip, persist flip + verbatim Other:** Tasks 1–3 tests. ✓
