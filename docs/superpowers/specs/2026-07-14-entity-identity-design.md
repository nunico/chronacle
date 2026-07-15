# Entity Identity — Aliases, Fuzzy Resolution, Merge, and Rename-Safe Vault Keys

**Date:** 2026-07-14
**Status:** Approved (design). Implementation not started.
**Area:** `crates/chronacle-db/src/schema/`, `crates/chronacle-extraction/`
(`wikilink/`, `codex_service/lint.rs`, `entity_service/`),
`crates/chronacle-vault/` (`keys.rs`, `render.rs`, `reconcile.rs`),
`crates/chronacle-domain/`, `apps/desktop/src-tauri/src/commands/`,
`apps/desktop/src/`
**Roadmap:** Tranche 6. Follows tranche 5 (inbound vault sync, PR #31). The
Codex letter scheme (A–E) is fully delivered; this opens new Phase-3 ground.
**ADR:** Introduces **ADR-012 (Entity Identity)**. Answers Open Question #1 of
ADR-009 ("does an entity merge operation exist?" — it did not; now it does).

## Problem

Chronacle has no concept of entity identity beyond an exact name string. An
entity **is** its name, so every variant of that name is a different thing.
Two symptoms, one cause:

1. **Duplicate entities under name variants.** "The Free League" and "Free
   League" are two rows. `lint_duplicates`
   (`codex_service/lint.rs:245`) groups on `(table, name.trim().to_lowercase())`,
   so the two never collide and the duplicate is **never reported**. The
   finding it would write hardcodes `"similarity": 1.0` — the payload was
   shaped for fuzzy matching that was never built. And even when a duplicate
   _is_ reported, the Maintenance card offers only "Open A" / "Open B":
   **there is no merge operation anywhere in the codebase.**

2. **Partial and inflected links do not resolve.** Wikilink resolution
   (`wikilink/mod.rs:112`) is whole-string equality:
   `name.to_lowercase() == lower`. `[[The Quassars]]` can only match an entity
   literally named "The Quassars", so it fails and files a `broken_wikilink`
   even though "The Quassar Family" is sitting right there.

Both reduce to the same missing primitive: **a name variant is not a new
entity, it is another name for an existing one.**

A third problem is a prerequisite rather than a symptom: **renaming is not
vault-safe** (see "Rename safety" below), and merge renames things constantly.

## Decisions locked during design review

Decided with the maintainer on 2026-07-14; not open:

1. **Aliases are the single mechanism.** Everything in this tranche either
   populates aliases (auto-resolve, confirm-a-suggestion, merge) or honors
   them (link resolution, duplicate detection).
2. **Resolution is tiered:** auto-resolve when a fuzzy match is confident _and
   unambiguous_; otherwise suggest a candidate and let the GM confirm.
3. **An auto-resolve persists its alias** and is listed in Maintenance as
   reviewable/undoable. Nothing happens behind the GM's back, but the GM is
   never _required_ to act.
4. **Similarity is normalized string similarity, hand-rolled.** No new crate,
   no ADR for a dependency, no LLM in the loop. Deterministic and explainable.
5. **Merge is a full merge** with a field-by-field dialog. Edges and aliases
   always union; nothing is silently destroyed.
6. **Rename safety ships in this tranche**, including campaign rename — merge
   is unsafe without it. **Correction (2026-07-15):** this premise was false;
   see "Rename safety (the prerequisite)" below. Rename was already safe.
   Campaign rename and file-move logic were dropped; only the frontmatter
   alternate-names seam was built.

## Domain model changes

### `aliases` on the eight entity tables + `rule_entry`

```surql
DEFINE FIELD OVERWRITE aliases ON TABLE <t> TYPE array<string> DEFAULT [];
```

for each of `npc`, `location`, `faction`, `creature`, `item`, `event`,
`player_character`, `misc`, and `rule_entry`.

GM-owned (never compiler-owned): a merge or a confirmed suggestion writes it,
the GM may edit it directly, and it round-trips through the vault.

> **LANDMINE — `DEFAULT` never backfills, and it breaks WRITES.** A
> SCHEMAFULL record re-validates _every_ field on _any_ write, and `NONE`
> satisfies neither `array<string>` nor `string | NULL`. A pre-migration row
> with no `aliases` value would fail **every subsequent write**, not merely
> read as empty. `aliases` MUST be added to `backfill_unset_fields` in
> `chronacle-db/src/schema/mod.rs` in the same PR as the `DEFINE FIELD`, and a
> test must seed a row _before_ `run_migrations` and then write to it. This
> exact bug shipped green in tranche 5 and broke inbound sync for every real
> campaign.

### Alias uniqueness

An alias must not collide, **within its resolution scope**, with any other
entity's name or alias. Otherwise tier 2 (below) stops being deterministic and
the same link resolves differently depending on row order.

Enforcement is **validate-then-lint**, not a DB constraint (scope is a graph
traversal, not a column): `entity_service` rejects a colliding alias on write
with a typed error, and the lint pass files a `alias_collision` finding for any
collision that predates the check or arrives via the vault. A colliding alias
is **ignored** by resolution (falls through to the next tier) rather than
picking a winner arbitrarily.

### New lint kinds

| Kind              | Payload                                 | Meaning                                                                                                             |
| ----------------- | --------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `alias_collision` | `{ alias, a, b }`                       | Two entities in scope claim the same alias/name.                                                                    |
| `auto_alias`      | `{ entity, alias, similarity, source }` | An alias written by tier-4 auto-resolve. Reviewable/undoable. Informational — resolved by acknowledging or undoing. |

`broken_wikilink` gains a `candidates: [{ id, name, similarity }]` array
(ranked, may be empty), which is what powers "did you mean …?".
`duplicate_entity` finally writes a **real** `similarity` instead of `1.0`.

## Name normalization

A pure function, `normalize(name) -> String`, in `chronacle-extraction`
(no I/O, exhaustively unit-tested):

1. Unicode case-fold and trim.
2. Strip a leading definite article (`the `). **Only leading, only `the`** —
   `A Cage of Iron` keeps its `A`, because dropping indefinite articles
   collapses too many distinct titles.
3. Strip possessives: trailing `'s` / `’s`.
4. Singularize a trailing `s` / `es` (a small conservative rule set, not a
   full stemmer — `Quassars` → `quassar`, but `Chaos` is left alone).
5. Collapse internal punctuation and whitespace to single spaces.

Normalization is **not** lossy for storage — it is only ever a lookup key. The
GM's exact spelling is preserved as name or alias.

## Resolution tiers

`resolve(name, scope)` stops at the first tier that hits:

| Tier | Match                           | Example                                    |
| ---- | ------------------------------- | ------------------------------------------ |
| 1    | Exact name (case-insensitive)   | today's behavior, unchanged                |
| 2    | Exact **alias**                 | a confirmed variant, deterministic forever |
| 3    | **Normalized** name or alias    | `Free League` ↔ `The Free League`          |
| 4    | **Fuzzy** over normalized forms | `The Quassars` → `The Quassar Family`      |

Tier 3 is what fixes the article/plural cases outright: both sides normalize to
the same key, so it is an _exact_ match on a normalized index — no scoring, no
threshold, no ambiguity.

Tier 4 is trigram similarity (Dice coefficient over character trigrams of the
normalized forms, with a containment bonus so `quassar` scores high against
`quassar family`). It **auto-resolves only when exactly one candidate clears
the threshold.** Two candidates above it means the link is genuinely ambiguous;
guessing would be the wrong kind of confident. A tier-4 hit:

- writes the linked text as an **alias** on the matched entity, so the next
  pass hits tier 2 — the fuzzy path runs **once per variant, ever**;
- files an `auto_alias` finding so the GM can review and undo it.

No tier-4 hit (or an ambiguous one) → `broken_wikilink` **carrying its ranked
candidates**, which Maintenance renders as "did you mean **The Quassar
Family**?" with a one-click _confirm_ that writes the alias.

> **Threshold tuning is a deliverable, not a constant pulled from the air.**
> Too loose silently welds distinct entities together; too tight never fires.
> The threshold is tuned against the maintainer's real campaign DB via a
> read-only dry-run (see Test strategy), and the chosen value is recorded in
> the ADR with the evidence.

> **LANDMINE — scope.** Resolution scope is already a graph traversal
> (`WikilinkScope::Campaign` chases `in_campaign` + `subscribes_to->in_collection`;
> `Collection` stays inside one collection — `wikilink/query.rs`). Aliases and
> fuzzy candidates MUST honor exactly the same scope, or a link in one campaign
> starts resolving to another campaign's entity.

## Duplicate detection and merge

**Detection** (`lint_duplicates`) becomes two-stage: group on the _normalized_
name (which catches Free League / The Free League with no scoring at all), then
fuzzy-compare the remaining pairs **within the same table and scope**, writing
the real similarity into the payload. Pairs below the threshold are not
reported — a lint inbox full of non-duplicates is worse than none.

**Merge** — `entity_service::merge(survivor, loser, choices)` — is the operation
that has never existed:

1. **Union `relates_to` and `mentioned` edges** onto the survivor (both
   directions), de-duplicating. **No edge is ever dropped**; a relationship is
   a fact about the world, not a stylistic choice.
2. **Union aliases**, and add the **loser's name** as an alias of the survivor
   — this is what makes every existing `[[Free League]]` keep working.
3. **Apply per-field choices** (`keep_survivor` | `keep_loser` | `keep_both`)
   for `summary` and `notes`. `keep_both` concatenates under a
   `## Merged from <name>` heading.
4. **Mark `codex_stale = true`.** The article is compiler-owned and will be
   regenerated from the merged facts; merging two articles textually would
   produce prose no compiler wrote and no citation supports.
5. **Re-embed** the survivor (its name/summary/notes changed).
6. **Soft-delete the loser** (`vault_deleted = true`) and remove its vault file
   through the normal reconcile path — never a raw `DELETE`.
7. **Resolve** the `duplicate_entity` finding.

**Crash safety.** SurrealDB supports `BEGIN`/`COMMIT`, but the codebase uses no
multi-statement transaction anywhere today (verified: zero occurrences), and
merge also performs non-DB work — an embedding call and a vault file removal —
that no DB transaction can cover. So merge does not rely on atomicity: it is
ordered **edges first, soft-delete last**, and every step before the soft-delete
is idempotent and re-runnable. A crash mid-merge therefore leaves both records
alive with a _superset_ of edges — visibly unfinished, safe, and re-runnable.
The reverse order (delete first) could orphan edges permanently. If a later
tranche adopts transactions, steps 1–4 can be wrapped without reordering.

## Rename safety (the prerequisite)

> **Correction (2026-07-15):** everything below this note described a
> premise — that renaming an entity strands GM edits as an orphaned
> duplicate file — that this section's author (the maintainer) got wrong.
> It was written assuming vault keys are derived from the entity's **name
> slug**, and never checked against the actual reconcile implementation.
> Investigation during implementation of this tranche found the opposite:
> reconcile locates a record's vault file by the record **id** embedded in
> its frontmatter (`index.key_of`), not by a name-derived slug. Renaming an
> entity therefore already updates its file **in place** — the filename
> goes stale relative to the new name, but the content stays correct and
> Obsidian's own `[[links]]` still resolve via the `aliases:` frontmatter
> line. A rename with a concurrent, unsynced GM edit produces an ordinary
> conflict sidecar, exactly like any other concurrent edit — there is no
> delete-old + export-new path and no data-loss path to guard against.
>
> Consequently, the move/rename reconcile logic and campaign-rename command
> described below were **not built** — they would only have fixed a stale
> filename, not a safety bug, and were dropped as out of scope for this
> tranche (see "Out of scope"). The only thing that was actually
> load-bearing, and was built, is the frontmatter alternate-names seam (see
> the "Draft: addition to Your Vault" landmine below and ADR-012). The
> original (incorrect) text is left in place, unedited, for the history —
> do not treat anything past this note as a description of what shipped.

Vault keys derive from the **name slug** (`chronacle-vault/src/keys.rs:107`,
`slug(name)`), so renaming an entity — or merging, which renames by definition —
changes its vault key. Today reconcile sees an unfamiliar key and treats the
change as **delete-old + export-new**. For an untouched file that is fine. For a
file the GM edited outside the app, the base no longer matches, the orphan sweep
(correctly) refuses to delete it, and the result is a **stale duplicate file on
disk whose GM edits never land in the DB**. Merge would hit this constantly.

`vault_sync_state` already stores the last-synced `key` per record. Reconcile
gains a **move** decision: when a record's _computed_ key differs from its
_stored_ key and the record's identity is unchanged, it is a rename, not a death
and a birth:

- if the on-disk file at the old key still matches the base → **rename it on
  disk**, carry the merge base across, update the stored key. No export, no
  delete, no lost edits.
- if the on-disk file at the old key has **diverged** from the base (a GM edit
  we have not applied yet) → **apply the inbound edit first**, then move. The
  GM's edit is never collateral damage of a rename.
- if the old file is gone → plain export at the new key.

**Campaign rename** is the same machinery one level up: renaming a campaign
changes `campaigns/<slug>/` for every record beneath it, so every key moves at
once. It ships here because it is the same code path and is otherwise a
data-loss feature. The rename command renames the folder and re-keys the sync
state in one reconcile pass.

## UX

- **Entity detail:** an Aliases field (chips, add/remove). Editable directly.
- **Maintenance inbox:**
  - `broken_wikilink` cards gain **"Did you mean _X_?"** with a one-click
    confirm that writes the alias and re-resolves.
  - `duplicate_entity` cards gain **Merge**, opening a side-by-side dialog:
    choose survivor; per field keep A / keep B / keep both; a plain-language
    summary of what will happen ("12 relationships merged, 3 aliases kept, the
    codex article will be recompiled").
  - `auto_alias` cards are a quiet, collapsible list: "Chronacle linked _The
    Quassars_ → _The Quassar Family_ (91%)" with **Undo**.
  - `alias_collision` cards link both claimants.
- **Settings → Campaign:** rename, with a warning that the vault folder moves.
- **Vault frontmatter:** aliases are visible and editable in Obsidian.

### UI hints (in-app, not manual-only)

Every surface below carries a one-line explanation in place, because a GM who
has to open the manual to understand a merge dialog has already been failed by
it:

- Aliases field: _"Alternate names this is known by. Links using any of them will
  find this entity."_
- Merge dialog: a plain-language consequence line — _"12 relationships merged,
  3 alternate names kept, the codex article will be rewritten."_
- Auto-linked list: _"Chronacle made these links on its own. If one is wrong,
  undo it — the link will go back to asking you."_
- Campaign rename: _"Your vault folder will be renamed too. Close Obsidian
  first if it has this folder open."_

## Documentation plan (GM-facing)

Every interactive surface ships manual copy **in the same PR**, per the tranche-5
lesson: the vault conflict lifecycle was unusable until it was explained for
non-technical GMs. These paragraphs are drafted here so they transfer verbatim
into `docs/user-guide.md` rather than being reinvented at the end. Voice: second
person, no jargon, explain the _why_, never the implementation.

Placement: a new **"Names and duplicates"** chapter after "The Codex", plus an
addition to the existing "Managing Campaigns" chapter for rename.

> **Correction (2026-07-15):** the "Names and duplicates" chapter and the
> "Your Vault" addition shipped as drafted below. The "Managing Campaigns"
> addition did not — campaign rename was dropped (see "Rename safety"
> above) and its draft copy below describes a feature that was never built.

---

### Draft: Names and duplicates

**When the same thing has two names**

Your world is full of things that go by more than one name. The Free League and
the Free League. The Quassars and the Quassar Family. You know these are the
same, but Chronacle starts out taking every name literally — to it, "The Free
League" and "Free League" look like two different factions, and a link to
[[The Quassars]] doesn't find the Quassar Family at all.

You fix this by giving something **alternate names**. Open any entity and you'll
find an _Alternate names_ field. Anything you put there works exactly like the
entity's real name: links pointing at it land here, and Chronacle stops treating
it as a stranger. You only ever have to do this once per name — it sticks.

**Links that Chronacle sorts out by itself**

Most of the time you won't have to do anything. When you write a link that
doesn't match anything exactly, Chronacle looks for the obvious answer. If
there's exactly one thing it's clearly pointing at — [[The Quassars]] when the
Quassar Family is the only Quassar anything in your campaign — it makes the link
and remembers the name for next time.

It only does this when there's a single sensible answer. If two things could
both be what you meant, it won't guess: it asks.

Everything Chronacle links on its own shows up in **Maintenance** under
_Auto-linked_. You never have to look at that list — it's there so nothing
happens behind your back. If it ever gets one wrong, hit **Undo** and it will
ask you next time instead of deciding.

**Links Chronacle isn't sure about**

When a link doesn't match anything and there's no obvious answer, it shows up in
Maintenance as a broken link — as it does today — but now with a suggestion:
_"[[The Quassars]] — did you mean **The Quassar Family**?"_ One click and the
name is added, the link works, and every other link using that name works too.

If the suggestion is wrong, ignore it. A broken link is only a broken link; it
never invents a connection you didn't ask for.

**Merging two entries that are the same thing**

If you've ended up with two entries for one thing — it happens easily when a
rulebook says "the Free League" and your session notes say "Free League" —
Chronacle will spot it and offer to merge them.

You'll see them side by side. Pick which one to keep, and for each piece of
writing — the summary, your notes — choose which version survives, or keep both.
Relationships are always kept from both sides: if one entry knew about a
connection the other didn't, the merged entry knows about it too. Nothing gets
quietly dropped.

The name of the entry you didn't keep isn't lost either — it becomes one of the
merged entry's alternate names. Every link you ever wrote using it keeps working.

The merged entry's codex article is marked for rewriting, because it was written
from half the facts. Recompile when you're ready.

---

### Draft: addition to "Managing Campaigns"

**Renaming a campaign**

You can rename a campaign from its settings. If you use vault sync, the folder
holding that campaign's files is renamed to match — your notes move with it,
edits and all, and nothing is lost.

One thing worth doing first: if you have that folder open in Obsidian, close it
before you rename. Obsidian doesn't always cope gracefully with a folder being
renamed underneath it. Chronacle handles the rename safely either way; it's
Obsidian we're being careful of.

---

### Draft: addition to "Your Vault"

**Alternate names in your vault files**

Each file has an `aliases:` line near the top. That's the entity's alternate names,
and you can edit it in Obsidian directly — add one and Chronacle picks it up on
the next sync, exactly as if you'd typed it into the app. Obsidian uses the same
line for its own linking, so a name you add here works in both places at once.

Leave the entity's own name in the list. It's what makes your `[[links]]` in
Obsidian find the file.

> **LANDMINE — the frontmatter `aliases` key already exists and means
> something else.** `render.rs:29` writes `aliases: vec![e.name.clone()]` — a
> _derived_ single-element list whose only job is to make Obsidian resolve
> `[[Display Name]]` to a slug-named file. It is not GM data. Naively storing
> entity aliases in the same key means **export clobbers the GM's aliases** and
> **inbound reads the entity's own name back as a GM alias**. The seam is
> explicit: export writes `[name] ∪ aliases`; inbound parses GM aliases as
> `frontmatter.aliases − name` (case-insensitively). Both directions get a
> round-trip test, and the round-trip must be idempotent (export → parse →
> export produces byte-identical output).

## Test strategy

- **Normalizer:** table-driven over the real cases (`The Free League`,
  `Free League`, `The Quassars`, `The Quassar Family`, `Chaos`, `A Cage of
Iron`) + property tests: idempotent (`normalize(normalize(x)) == normalize(x)`),
  and never empty for non-empty input.
- **Similarity:** table-driven with asserted score bands, including **negative**
  cases that must _not_ match (`The Legion` vs `The Legionnaire's Rest` are a
  faction and a tavern — a false merge here is data loss).
- **Resolution:** one test per tier, plus the two that matter most — _ambiguity
  does not auto-resolve_ (two candidates above threshold → suggestion, not a
  guess), and _scope is honored_ (a fuzzy candidate in another campaign is
  invisible).
- **Merge:** no edge is lost (union asserted in both directions); the loser's
  name resolves to the survivor afterwards; a crash after step 1 leaves a
  recoverable state.
- **Vault rename:** the load-bearing one — **a GM-edited file survives a rename
  with its edits intact.** This is the test that catches the stale-orphan bug.
  Plus: rename with an untouched file moves it; campaign rename moves the whole
  subtree; frontmatter alias round-trip is idempotent.
- **Pre-migration:** seed a row _before_ `run_migrations`, then **write to it**
  (see the `DEFAULT` landmine).
- **Dry-run against real data:** a read-only command that reports what fuzzy
  detection _would_ do over the maintainer's actual campaign DB, run before the
  threshold is fixed. A green suite is not evidence this works — fresh fixtures
  cannot tell us whether the threshold is right for a real world.
- **BDD (ADR-011):** `.feature` scenarios for confirm-a-suggestion, merge, and
  campaign rename.

## Risks & tradeoffs

- **Tier 4 is where trust is won or lost.** A confident wrong match is
  indistinguishable from a right one. Mitigations: auto-resolve only when
  _unambiguous_; persist + surface every auto-alias with Undo; tune the
  threshold on real data; prefer a missed match (which degrades to a
  suggestion) over a false one (which silently corrupts the graph).
- **Merge is destructive by nature.** Mitigated by unioning everything that is
  cheap to keep (edges, aliases), never hard-deleting, and requiring explicit
  per-field choices.
- **Normalization is English-centric** (leading "the", trailing "s"). Accepted:
  the corpus is English TTRPG material. The normalizer is a single pure function
  and can grow rules without touching its callers.
- **No synonym detection.** "The Legion" will not match "Iron Host". This is
  deliberate — those are not name variants, they are facts about the world, and
  inferring them needs an LLM and produces confident nonsense.

## Out of scope

In-app vault conflict _resolution_ UI; undelete/trash for soft-deleted records;
id-less file adoption; GM-secret detection; `/extract-all` rework;
cross-encoder reranking. All remain on the Phase-3 list.

**Correction (2026-07-15):** vault rename/move reconcile logic and campaign
rename (both originally planned as prerequisites for merge, see "Rename
safety" above) are also out of scope, dropped after the rename-safety
premise they were built to fix was found to be false — renames were already
data-safe. Only the frontmatter alternate-names seam was needed and was
built (see ADR-012).

## Open questions

1. **Tier-4 threshold value** — resolved empirically during implementation via
   the dry-run against real campaign data; recorded in ADR-012.
2. **Should `rule_entry` participate in fuzzy duplicate detection**, or only in
   alias resolution? Rules dedupe on `(collection, name)` today. Proposed:
   aliases yes, fuzzy duplicate detection deferred — rule names are more
   formulaic and a false merge of two rules is worse than a duplicate.
