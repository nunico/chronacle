# Compiled World Model — The Codex (A2–C series)

**Date:** 2026-07-03
**Status:** Approved (design). Implementation not started.
**Area:** `crates/chronacle-db/src/schema/`, `crates/chronacle-extraction/`,
`crates/chronacle-retrieval/`, `crates/chronacle-domain/`,
`apps/desktop/src-tauri/src/commands/`, `apps/desktop/src/`
**Roadmap:** Continues the compiled-world-model letter scheme started by
PR-A1 (ADR-010, `2026-07-02-compiled-world-model-a1-design.md`). This spec
covers A1b and the A2, B, and C series in detail and sketches D. It is the
"b-compile" plan the A1 spec deferred to, plus everything around it.
**ADR:** Introduces ADR-009 (Compiled World Model — Codex).

## Problem

Chronacle today is `extract → entity graph → answer`. Every answer is
re-derived at question time from raw chunks plus thin entity summaries.
Nothing durable sits between extraction and answering:

- There is **no persistent compiled world model**. The entity graph stores
  names, one-line summaries, and edges — not compiled, citable articles.
- There is **no write-back**. A great cited answer in chat, or a session's
  worth of notes, changes nothing in the knowledge layer. Durable results
  evaporate.
- There is **no linting**. Broken `[[wikilinks]]`, duplicate entities,
  stale summaries, and scope violations accumulate silently. (`lint_finding`
  exists since A1 but has one producer and no UI.)
- Extraction is **setting-oriented only**. Rules — the other half of what a
  GM asks about — are never compiled; they are answered from raw chunk
  retrieval every time.

This is the gap the LLM Wiki pattern (Karpathy) fills: compile knowledge
into durable, provenance-carrying articles once; answer from the compiled
layer; write durable results back; lint for drift.

## Decisions locked during design review

These were decided with the maintainer on 2026-07-03 and are not open:

1. **Adopt the A2–D letter scheme** from the A1 spec; this spec details
   A1b–C2 and sketches D.
2. **Setting articles live on the entity tables** (new machine-owned
   fields); **rules get a new `rule_entry` aggregate**. No `wiki_page`
   table.
3. **Write-back goes through a review queue** (`codex_proposal`); nothing
   writes to the compiled layer silently. A chat answer may yield
   **several** targeted proposals.
4. **Compilation is manual with staleness markers** — an explicit action
   per collection; badges show what is pending. No background or automatic
   compilation.
5. **The layer is named "Codex"** in UI and code (`codex_*`), avoiding
   collision with the existing `wikilink` module.
6. **`codex_article` is a separate machine-owned field**; user `summary`
   and `notes` are never machine-overwritten. Write-back may target `notes`
   only via an accepted proposal.
7. **Rules compile reads `rules` and `supplement` sources**, with
   chunk-level LLM classification inside them (supplements mix rules and
   lore).
8. **`rule_entry` bodies are not user-editable.** Each entry gets a
   GM-owned `notes` field, and a "redo with objections" action recompiles
   the entry with the GM's objection injected into the prompt.
9. **`rule_entry.category` is a closed enum** (see Domain model).
10. **Plan depth:** full detail through C2; D-series (vault sync) sketched
    only.

## Bounded contexts (DDD)

| Context             | Home                                          | Change                                                                                                    |
| ------------------- | --------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Ingestion           | `chronacle-ingestion`                         | Unchanged pipeline; gains one duty — mark staleness when a source lands in a collection.                   |
| Knowledge Graph     | `chronacle-extraction` (entity/relation half) | Unchanged shape; becomes a feeder of the Codex; gains scope validation on relation creation.               |
| **Codex** (new)     | `chronacle-extraction::codex_service`         | Owns compiled knowledge: entity articles, rule entries, proposals, lint passes. New module, **no new crate**. |
| Retrieval           | `chronacle-retrieval`                         | Consumes compiled knowledge first, raw chunks last (RULES → CODEX → ENTITIES → CHUNKS).                    |

Aggregates:

- **Entity** (existing root, 8 node tables) — gains a machine-owned
  *codex article* value object: `codex_article`, `codex_compiled_at`,
  `codex_stale`, `codex_sources`.
- **RuleEntry** (new root) — a discrete compiled rule, always owned by
  exactly one collection.
- **CodexProposal** (new root) — a pending write-back suggestion; the
  review queue.
- **LintFinding** (existing since A1) — gains new kinds and producers.

`codex_service` lives in `chronacle-extraction` because compilation has the
same dependency shape as extraction (chunks + entities + `Arc<dyn
LlmProvider>` + `Arc<dyn EmbeddingProvider>`). If the module outgrows the
crate, extracting a `chronacle-codex` workspace crate is a later, mechanical
refactor (workspace-internal crates need no ADR per architecture doc).

## Domain model changes

All schema changes extend `crates/chronacle-db/src/schema/002_wiki_layer.surql`,
additive only, every statement `DEFINE … OVERWRITE` (idempotent re-runs, per
repo convention and the schema-migrations-rerun-every-boot constraint).

### Entity tables — codex article fields (×8 tables)

On each of `npc`, `location`, `faction`, `creature`, `item`, `event`,
`player_character`, `misc`:

```
DEFINE FIELD OVERWRITE codex_article     ON <table> TYPE option<string> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_compiled_at ON <table> TYPE option<datetime> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_stale       ON <table> TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE codex_sources     ON <table> TYPE array<object> DEFAULT [];
```

- `codex_article` — compiled markdown article with inline
  `[Source: "<name>", p.N]` markers. Machine-owned: the compiler may
  regenerate it freely.
- `codex_sources` — provenance entries. Shapes by `kind`:
  `{ kind: "chunk", source, page_start, page_end }`,
  `{ kind: "session", session }`,
  `{ kind: "proposal", proposal }`.
- `codex_stale` — set by staleness producers (below); cleared by compile.

### `rule_entry` (new table)

```
DEFINE TABLE OVERWRITE rule_entry SCHEMAFULL;
DEFINE FIELD OVERWRITE collection  ON rule_entry TYPE record<collection>;
DEFINE FIELD OVERWRITE name        ON rule_entry TYPE string;
DEFINE FIELD OVERWRITE category    ON rule_entry TYPE string
    ASSERT $value IN ['mechanic', 'ability', 'state', 'procedure', 'resource', 'statistic', 'entry'];
DEFINE FIELD OVERWRITE body        ON rule_entry TYPE string;
DEFINE FIELD OVERWRITE notes       ON rule_entry TYPE string | NULL DEFAULT NULL;
DEFINE FIELD OVERWRITE page_refs   ON rule_entry TYPE array<object> DEFAULT [];
DEFINE FIELD OVERWRITE sources     ON rule_entry TYPE array<object> DEFAULT [];
DEFINE FIELD OVERWRITE compiled_at ON rule_entry TYPE datetime;
DEFINE FIELD OVERWRITE stale       ON rule_entry TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE embedding   ON rule_entry TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD OVERWRITE embed_model ON rule_entry TYPE string | NULL DEFAULT NULL;
DEFINE FIELD OVERWRITE created_at  ON rule_entry TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at  ON rule_entry TYPE datetime DEFAULT time::now();
DEFINE INDEX OVERWRITE rule_entry_collection_idx ON rule_entry COLUMNS collection;
DEFINE INDEX OVERWRITE rule_entry_name_idx       ON rule_entry COLUMNS collection, name UNIQUE;
DEFINE INDEX OVERWRITE rule_entry_embedding_idx  ON rule_entry FIELDS embedding MTREE DIMENSION 768 DIST COSINE;
```

Category semantics (closed enum, chosen by the maintainer):

| category    | meaning                                                                          |
| ----------- | -------------------------------------------------------------------------------- |
| `mechanic`  | a discrete rule or subsystem (initiative, opposed checks, downtime)               |
| `ability`   | a named capability an actor can use (spell, feat, technique, power, maneuver)     |
| `state`     | a condition or status affecting an actor (poisoned, exhausted, hunted)            |
| `procedure` | a step-by-step sequence (character creation, long rest, chase scene)              |
| `resource`  | a countable in-play thing with rules attached (hit points, mana, stress, ammo)    |
| `statistic` | a numerical value used or modified in or by another rule (armor class, speed)     |
| `entry`     | freeform fallback (equivalent to the existing `misc` idiom on entities)           |

Editability: `body`, `category`, `page_refs`, `sources` are compiler-owned.
`notes` is GM-owned and survives recompiles. The GM's correction path for a
wrong entry is **redo-with-objections**: a recompile of that single entry
with the GM's objection text injected into the prompt; the objection is
recorded in `sources` as `{ kind: "objection", text, at }` so later
recompiles keep honoring it.

- `rule_entry` deliberately does **not** reuse the entity `misc` table —
  rules must never leak into entity retrieval, graph views, or extraction
  dedup, and they carry fields (category, page_refs) entities do not.

### `codex_proposal` (new table)

```
DEFINE TABLE OVERWRITE codex_proposal SCHEMAFULL;
DEFINE FIELD OVERWRITE kind        ON codex_proposal TYPE string
    ASSERT $value IN ['entity_article_update', 'entity_notes_update',
                      'rule_entry_update', 'new_entity', 'new_rule_entry'];
DEFINE FIELD OVERWRITE target      ON codex_proposal TYPE option<record> DEFAULT NONE;
DEFINE FIELD OVERWRITE collection  ON codex_proposal TYPE record<collection>;
DEFINE FIELD OVERWRITE campaign    ON codex_proposal TYPE option<record<campaign>> DEFAULT NONE;
DEFINE FIELD OVERWRITE payload     ON codex_proposal TYPE object;
DEFINE FIELD OVERWRITE origin      ON codex_proposal TYPE object;
DEFINE FIELD OVERWRITE status      ON codex_proposal TYPE string DEFAULT 'pending'
    ASSERT $value IN ['pending', 'accepted', 'rejected'];
DEFINE FIELD OVERWRITE created_at  ON codex_proposal TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE resolved_at ON codex_proposal TYPE option<datetime> DEFAULT NONE;
DEFINE INDEX OVERWRITE codex_proposal_status_idx ON codex_proposal COLUMNS status;
```

- `payload` — `{ proposed_text, rationale }` (update kinds) or a full
  draft object (`new_*` kinds).
- `origin` — `{ kind: "chat", message }`, `{ kind: "session", session }`,
  or `{ kind: "manual" }`.
- `target` is unset for `new_*` kinds.

### `lint_finding` — new kinds (C2)

No schema change (the table is deliberately loose). New `kind` values and
payload shapes, documented in the migration file comment block:

| kind               | producer                              | payload sketch                              |
| ------------------ | ------------------------------------- | ------------------------------------------- |
| `orphaned_edge`    | campaign delete-convert (exists, A1)  | unchanged                                   |
| `scope_violation`  | lint pass over existing edges/links   | `{ edge/link, from, to, from_collection, to_collection }` |
| `broken_wikilink`  | lint pass over entity notes/articles  | `{ entity, link_text }`                     |
| `stale_article`    | lint pass (aggregates `codex_stale`)  | `{ entity, reason }`                        |
| `duplicate_entity` | lint pass (name similarity in scope)  | `{ a, b, similarity }`                      |

LLM-driven `contradiction` detection is **explicitly deferred** (expensive,
noisy); it is future work, not C2.

## Reference rules (scope model)

The two collection types come from A1/ADR-010:

- **campaign-bound** — `owner_campaign` set; owned by exactly one campaign;
  never subscribable by other campaigns.
- **regular** — `owner_campaign` unset; subscribable by any campaign.

Reference rules, where "reference" means `relates_to` edges, `[[wikilinks]]`,
codex article provenance, and rule citations:

1. Content in a **campaign-bound** collection may reference content in any
   collection its owning campaign **subscribes to** (plus itself).
2. Content in a **regular** collection may reference **only content in that
   same collection**.

Enforcement points (all three, in this order of strength):

1. **Validate-on-write** — `entity_service` relation creation computes both
   endpoints' collections and the allowed set, rejecting violations with a
   typed error. Applies to every path: UI commands, extraction persist,
   seed extraction. Extraction bulk paths degrade gracefully: a rejected
   edge is skipped and logged as a `scope_violation` lint finding rather
   than failing the whole run.
2. **Scoped compilation** — the compiler's provenance retrieval is limited
   to the allowed collection set for the thing being compiled, so articles
   and rule entries physically cannot cite out-of-scope material.
   - Compiling a **campaign-bound** collection retrieves across the owner
     campaign's full subscription set.
   - Compiling a **regular** collection retrieves from that collection only.
3. **Lint pass** — `scope_violation` findings for pre-existing data
   (edges/links created before enforcement landed).

SurrealQL note: every KNN query that must respect scope uses an
**explicit-id array** for collection filtering, never `id IN (SELECT …)` —
the known MTREE + subquery composition pitfall (returns 0 rows silently).

## Compile pipelines

Both pipelines are manual (per-collection "Compile" action), incremental by
staleness, emit progress events mirroring `ExtractionProgress`, and reuse
`llm_complete` / `batch_passages` from `extraction_service`.

### Setting compile (B1) — entity articles

Per collection, for each entity with `codex_stale = true` **or**
`codex_article = NONE` (a "Recompile all" variant ignores staleness):

1. Gather context: KNN top-k chunks by the entity's embedding within the
   allowed collection set; the entity's `summary`, `notes`, graph
   neighborhood (1 hop, names + `rel_type` only); accepted proposals
   targeting it.
2. LLM writes or refreshes the markdown article with inline
   `[Source: "<name>", p.N]` markers. The prompt forbids inventing facts
   not present in the supplied context and instructs `[[Entity Name]]`
   wikilinks for in-scope entities.
3. Persist: `codex_article`, `codex_sources`, `codex_compiled_at = now`,
   `codex_stale = false`.
4. Re-embed the entity over `name + summary + article` (see Risks —
   embedding semantics change).

### Rules compile (B2) — rule entries

Per collection, over chunks whose `source_type ∈ {rules, supplement}`:

1. Batch chunks (`batch_passages`, existing token budget).
2. LLM pass per batch: classify chunk content, emit zero or more draft
   rule entries `{ name, category, body, page_refs }`. Supplement chunks
   that are pure lore emit nothing.
3. Dedup-or-merge by `(collection, name)` (the UNIQUE index backs this) —
   same discipline as entity extraction dedup. A merge marks the entry
   recompiled, preserves `notes`, and unions `page_refs`.
4. Embed each entry over `name + category + body`.

**Redo with objections** (single entry): recompile step 2–4 for one entry
with the GM's objection text injected into the prompt and appended to
`sources` as an `objection` record so future recompiles keep honoring it.

### Staleness producers

| event                                        | effect                                                             |
| -------------------------------------------- | ------------------------------------------------------------------ |
| source ingested into a collection            | all entities in that collection `codex_stale = true`; rules of the collection `stale = true` if `source_type ∈ {rules, supplement}` |
| extraction run touches an entity             | that entity `codex_stale = true`                                   |
| entity `notes`/`summary` edited              | that entity `codex_stale = true`                                   |
| proposal accepted against an entity          | article updated directly, so **not** stale; provenance appended    |
| session notes saved (C1)                     | mentioned entities `codex_stale = true` + proposals created        |

Staleness is coarse and cheap by design: a false-positive stale mark costs
one incremental recompile; a missed one is caught by the `stale_article`
lint pass.

## Retrieval integration (B3)

`agent_service::stream_response` gains one block and enriches another.
Prompt block order becomes **RULES → CODEX → ENTITIES → CHUNKS**:

1. **RULES** (new): KNN top-5 `rule_entry` across the campaign's subscribed
   collections (explicit-id array filter). Rendered with `category`,
   `body`, and page refs so answers cite book + page. GM `notes` on an
   entry are included, labeled as table rulings.
2. **CODEX** (enriched ENTITIES): `fetch_entity_context` already retrieves
   relevant entities; entities with a `codex_article` contribute an article
   excerpt (char-budgeted) instead of just `summary`.
3. **CHUNKS** (existing top-15 KNN): unchanged, now positioned as raw
   evidence below the compiled layers.

Each block has a character budget so the compiled layers cannot starve
chunk evidence. Citation parsing (`citation.rs`) already recognizes
`[Source: …]` markers; rule/article citations reuse the same format, so no
frontend citation change is required in B3.

No-campaign chat (`campaign_id = None`) skips RULES and CODEX exactly as it
skips entity context today.

## Write-back (C1)

Producers create `codex_proposal` rows; nothing mutates the compiled layer
directly:

- **Chat → "Save to Codex"** on an assistant message: an LLM pass distills
  the answer into **several targeted proposals** (per decision №3) —
  entity article/notes updates, new entities, rule entry updates — each
  with `origin = { kind: "chat", message }`.
- **Session notes saved**: an LLM pass over the notes proposes entity
  article updates and new entities; also marks mentioned entities stale.
- **Manual**: the GM can draft a proposal from an entity or rule entry
  page (small escape hatch, same table).

Review flow: the Maintenance inbox lists pending proposals with a
side-by-side diff preview. **Accept** applies the change, appends
provenance (`{ kind: "proposal", proposal }`), re-embeds the target, and
resolves the row. **Reject** resolves the row untouched. Proposals to
`entity_notes_update` are the *only* path by which machine text ever
reaches a user-owned field, and only via explicit accept.

## Linting (C2)

- **Inline producers** keep working as they land (A1's `orphaned_edge`;
  A2b's extraction-path `scope_violation`).
- **Manual lint pass** ("Check collection" / "Check campaign") runs the
  detectors in the lint-kind table above. Pure-Rust detectors only in C2
  (no LLM cost): scope scan, wikilink resolution against in-scope entity
  names, staleness aggregation, name-similarity duplicates.
- **Resolve actions** per kind in the UI: re-scope or delete edge
  (`scope_violation`), open entity (`broken_wikilink`), compile
  (`stale_article`), merge entities (`duplicate_entity` — reuses existing
  entity merge if present, else links to both).

Lint findings and proposals share one **Maintenance inbox** UI (two tabs) —
both are review-and-resolve queues; one sidebar badge sums both.

## UX

Principle: no new top-level concepts beyond "Codex" and one inbox. Compile
is a button where the content lives; review is one inbox; everything else
is badges.

- **Collection view:** "Compile" button with staleness badge
  ("3 sources uncompiled · 12 entities stale"); progress UI mirrors the
  existing extraction progress pattern. New **Rules** tab: rule entries
  grouped by category, name search, entry detail with body, page refs,
  GM notes editor, and "Redo with objections…" action.
- **Entity detail:** read-only **Codex Article** section (rendered
  markdown, wikilinks clickable) visually distinct from the editable Notes
  field; per-entity "Recompile" action; stale badge.
- **Chat:** "Save to Codex" action on assistant messages; toast links to
  the inbox ("4 proposals created").
- **Sidebar:** **Maintenance** item with pending-count badge (proposals +
  unresolved lint findings).
- **Campaign delete (A1b):** the already-designed two-mode dialog
  (cascade vs. convert-to-regular) — finishing approved A1 work.

## Delivery plan — PR slicing

Ground rules: every PR is a feature branch that does **not track main**
(`git checkout -b <branch> --no-track`), subagent-driven, ≤ ~800 lines,
tests ship in the same PR, TDD-ordered (failing tests first), no new
external crates anywhere in the series, green CI before merge.

| PR  | Branch                        | Content                                                                                     |
| --- | ----------------------------- | ------------------------------------------------------------------------------------------- |
| A1b | `feat/a1b-two-mode-delete-ui` | Frontend two-mode delete dialog; make `on_owned_collection` required; ADR-010 status note    |
| A2a | `feat/a2a-codex-schema`       | Schema: entity codex fields ×8, `rule_entry`, `codex_proposal`, lint-kind docs; ADR-009      |
| A2b | `feat/a2b-staleness-scope`    | Staleness producers (ingestion, extraction, entity edits); scope validation in `entity_service` with lint fallback on bulk paths |
| B1a | `feat/b1a-setting-compile`    | `codex_service::compile_collection` (articles) + progress events + Tauri command             |
| B1b | `feat/b1b-compile-ui`         | Compile button + staleness badges; Codex Article section on entity detail                    |
| B2a | `feat/b2a-rules-compile`      | Rules pipeline: classification, dedup-or-merge, redo-with-objections backend                 |
| B2b | `feat/b2b-rules-ui`           | Rules tab: category groups, search, entry detail, notes editor, redo action                  |
| B3a | `feat/b3a-rules-retrieval`    | RULES block: `rule_entry` KNN + prompt assembly + budgets                                    |
| B3b | `feat/b3b-codex-retrieval`    | Article excerpts in entity context; block ordering + budget tests                            |
| C1a | `feat/c1a-proposals-backend`  | Proposal producers (chat distill, session-notes pass) + accept/reject service                |
| C1b | `feat/c1b-inbox-ui`           | Maintenance inbox (proposals tab): diff preview, accept/reject, sidebar badge                |
| C2a | `feat/c2a-lint-pass`          | Lint detectors (scope, wikilink, stale, duplicate) + manual pass command                     |
| C2b | `feat/c2b-lint-ui`            | Maintenance inbox (findings tab): per-kind resolve actions                                   |

Dependency chain: A1b is independent. A2a → A2b → {B1a → B1b, B2a → B2b} →
{B3a, B3b} → {C1a → C1b, C2a → C2b}. B1 and B2 can proceed in parallel
after A2b; C1 and C2 in parallel after B3.

### D-series (sketch only — not planned here)

Vault sync (ADR-008) export of codex articles and rule entries as markdown;
player-safe export gated on Phase-3 AI-detected GM-secret flags. Planned
when the C series has landed and ADR-008 implementation starts.

## BDD scenarios (acceptance criteria per series)

**A1b**

- Given a campaign with an owned collection, when the GM deletes the
  campaign, then a dialog offers "Delete campaign and its notes",
  "Keep notes as a regular collection", and "Cancel", and the backend
  receives the matching mode; omitting the mode is a command error.

**A2**

- Given a fresh database, when migrations run twice, then no data is lost
  and all new tables/fields exist (idempotency).
- Given an entity in a regular collection A and an entity in regular
  collection B, when a relation between them is created via any service
  path, then the write is rejected (or skipped + linted on bulk paths).
- Given a campaign subscribed to collection B, when an entity in its
  campaign-bound collection relates to an entity in B, then the write
  succeeds.
- Given a new source ingested into a collection, then every entity in that
  collection is marked `codex_stale`.

**B1**

- Given a collection with extracted entities and indexed chunks, when the
  GM clicks Compile, then every stale/article-less entity gains a
  `codex_article` citing only in-scope sources, and staleness clears.
- Given a compiled collection with no changes, when the GM clicks Compile
  again, then no LLM calls are made (nothing stale).
- Given a compiled entity, the GM's `notes` and `summary` are byte-for-byte
  unchanged by compilation.

**B2**

- Given a collection with a `rules` source, when rules compile runs, then
  discrete `rule_entry` rows exist with valid categories and page refs.
- Given a `supplement` source mixing lore and rules, then only rule-bearing
  content produces entries.
- Given an existing entry the GM disputes, when the GM submits "redo with
  objections", then only that entry is recompiled, the objection is stored,
  and the GM's `notes` survive.

**B3**

- Given compiled rules and articles, when the GM asks a rules question,
  then the answer cites book + page from a rule entry, and the system
  prompt contains blocks in RULES → CODEX → ENTITIES → CHUNKS order.
- Given a campaign with no compiled content, chat behaves exactly as today
  (regression guard).

**C1**

- Given an assistant answer, when the GM clicks "Save to Codex", then one
  or more pending proposals appear in the inbox, each targeted and
  diff-previewable; accepting one updates the target, appends provenance,
  and re-embeds; rejecting changes nothing.
- Given a saved session note mentioning a known NPC, then a proposal
  targeting that NPC exists and the NPC is marked stale.

**C2**

- Given an entity note containing `[[Nonexistent]]`, when the lint pass
  runs, then a `broken_wikilink` finding appears and resolves when the
  link is fixed or the entity created.
- Given two same-named NPCs in one scope, a `duplicate_entity` finding
  appears.

## Test strategy (TDD, per repo conventions)

- **Schema (integration, in-memory SurrealDB):** idempotency ×2 runs; field
  defaults; `rule_entry` category ASSERT rejects unknown values; UNIQUE
  `(collection, name)`.
- **Unit (Rust, `mockall` / `MockLlmProvider` / `MockEmbeddingProvider`):**
  compile prompt assembly; rules classification parsing (tolerant JSON,
  mirroring `parse.rs`); dedup-or-merge; staleness producers; scope
  validation matrix (campaign-bound × regular × subscribed ×
  unsubscribed); proposal distillation parsing; each lint detector.
- **Integration (`apps/desktop/src-tauri/tests/`):** compile → article
  persisted with provenance → entity re-embedded; rules compile over
  fixture chunks; proposal accept/reject round-trip; lint pass end-to-end;
  KNN scope filtering with explicit-id arrays (regression for the
  subquery pitfall).
- **Frontend (Vitest + testing-library):** compile button/progress; rules
  tab grouping + redo dialog; article section renders markdown and never
  offers editing; inbox diff + accept/reject; delete-campaign two-mode
  dialog.
- **E2E (Playwright backend, every PR):** ingest fixture → extract →
  compile → ask rules question → assert cited answer; save-to-codex →
  accept → recompiled answer reflects it.

## Documentation plan

| Doc                                            | Change                                                                                       | When    |
| ---------------------------------------------- | --------------------------------------------------------------------------------------------- | ------- |
| `docs/architecture.md`                         | New **ADR-009: Compiled World Model (Codex)**; data-model section (+codex fields, `rule_entry`, `codex_proposal`); RAG pipeline section (block ordering); phases table | A2a (ADR), then per-series updates |
| `docs/architecture.md` ADR-010                 | One-line status note when A1b lands (parameter now required)                                   | A1b     |
| `docs/superpowers/specs/` (this file)          | The approved design                                                                            | now     |
| `docs/superpowers/plans/`                      | One plan doc per series (`a2-…`, `b1-…`, `b2-…`, `b3-…`, `c1-…`, `c2-…`), written just-in-time before each series starts | rolling |
| `docs/user-guide.md`                           | New **"The Codex"** chapter: what compiling does and costs; setting vs. rules; the seven rule categories in GM terms; redo-with-objections; Save to Codex + inbox; staleness badges; collection types and what they may reference; lint checks | B1b, extended through C2b |

## Risks & tradeoffs

- **LLM cost of compiles.** A large collection compiles hundreds of
  entities. Mitigated by manual trigger, staleness-incremental default,
  visible progress with cancel, and per-run entity caps (mirroring
  `MAX_ENRICH`'s precedent).
- **Embedding semantics change.** Re-embedding entities over
  `name + summary + article` changes entity retrieval behavior vs. today.
  Accepted: articles are strictly richer signal. Embed-model identity rules
  (ADR-003) unchanged — same model, same field.
- **Prompt bloat in B3.** Four blocks compete for context. Mitigated by
  per-block char budgets with tests pinning them.
- **Rules misclassification in supplements.** Chunk-level classification
  will miss or over-extract; the correction path is redo-with-objections
  plus dedup-merge on recompile, not perfection on first pass.
- **Scope enforcement vs. existing data.** Validate-on-write may reject
  edges that flows created freely before. Bulk paths skip + lint instead of
  failing; the lint pass surfaces legacy violations for the GM to resolve.
  There is deliberately **no auto-fix**.
- **Derived-state safety.** `codex_article`, `rule_entry` bodies, and
  embeddings are all recompilable from chunks + accepted proposals +
  stored objections. The only unrecoverable user data in the new layer is
  `rule_entry.notes` and accepted-proposal edits to entity `notes` — both
  user-owned fields that compiles never touch. This is the core safety
  property; every compiler/lint bug short of a bad `notes`-proposal accept
  is recoverable by recompiling.
- **Two review queues in one inbox.** Proposals and lint findings have
  different resolution semantics; the shared inbox trades a little UI
  complexity for one place to look. If it muddles, splitting later is a
  frontend-only change.

## Open questions (remaining)

1. **Entity merge for `duplicate_entity` resolution** — does an entity
   merge operation already exist, or does C2b link to both entities and
   defer merge? (Resolve during C2 planning; does not affect earlier
   series.)
2. **Compile cancellation semantics** — cancel between entities (simple,
   proposed) or mid-LLM-call (needs stream abort plumbing)? Proposed:
   between entities in B1a; revisit if runs feel unresponsive.
3. **Per-run compile caps** — exact cap value and whether it is a setting
   (like `extraction_enrich_neighbors`) or a constant. Proposed: constant
   first, setting if users hit it.

## Resolved during design review

- Rules sources: `rules` + `supplement` with chunk-level classification.
- `rule_entry` editability: GM `notes` field + redo-with-objections; body
  compiler-owned.
- Chat write-back granularity: several targeted proposals per answer.
- Category taxonomy: the closed seven-value enum above (`statistic` added
  on review).
- Naming, data shape, review queue, manual compile, article-field
  separation, plan depth: see "Decisions locked".
