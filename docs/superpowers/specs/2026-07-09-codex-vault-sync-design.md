# Markdown Vault Sync — The Codex, D-series

**Date:** 2026-07-09
**Status:** Approved (design). Implementation not started.
**Area:** `crates/chronacle-core/`, `crates/chronacle-vault/` (new),
`crates/chronacle-providers/`, `crates/chronacle-domain/`,
`crates/chronacle-db/src/schema/`, `apps/desktop/src-tauri/src/commands/`,
`apps/desktop/src/`
**Roadmap:** The D-series sketched in
`2026-07-03-codex-compiled-world-model-design.md` (§"D-series (sketch only)").
The A/B/C series landed in PRs #4–#17. This is the final designated tranche of
the compiled-world-model feature.
**ADR:** Amends ADR-008 (Markdown Vault Sync). ADR-008 moves
`Proposed` → `Accepted` on D7.

## Problem

GMs want to read and edit their campaign notes in Obsidian, and to browse the
compiled codex as an interlinked wiki, with changes flowing both directions.
ADR-008 specified this. But ADR-008 was written against a data model that no
longer exists, and against a filesystem-only view of storage.

This spec reconciles ADR-008 with the codebase as built, and generalises its
storage assumption so a future backend (S3, WebDAV, HTTP) needs no engine
change.

## What ADR-008 got wrong

Five provisions of ADR-008 are unimplementable or undesirable as written. Each
is corrected below and amended in the ADR on D0.

1. **`entity` table with an `entity_type` field.** There is no `entity` table.
   There are eight per-type tables (`npc`, `location`, `faction`, `creature`,
   `item`, `event`, `player_character`, `misc`). A type change moves a record
   between tables, producing a **new record id** — which contradicts ADR-008's
   own stable-`id`-in-frontmatter contract. Its migration
   (`DEFINE FIELD vault_deleted ON entity`) has no table to attach to.
2. **Campaign-rooted directory tree.** Entities belong to _either_ a campaign
   _or_ a collection, exclusively (`unique_entity_campaign` and
   `unique_entity_collection`, each `UNIQUE` on `out`). Collections are shared
   across campaigns via `subscribes_to`. A campaign-rooted tree would duplicate
   every shared-collection entity into each subscribing campaign's folder, and
   an inbound edit to one copy would have no unambiguous write-back target.
   `rule_entry` is collection-scoped and has no home in that tree at all.
3. **`is_gm_only` in frontmatter, gated by `vault_include_gm_only`.** The
   manual `is_gm_only` flag was built and reverted. GM-secret content is
   passage-level and AI-detected in Phase 3. The setting has nothing to filter.
4. **"File I/O uses `tokio::fs` directly… No `FileStore` abstraction is
   needed."** This contradicts the standing "traits for all external deps"
   constraint, and — more concretely — leaves I/O failure and crash-recovery
   paths unreachable in tests.
5. **Timestamp-driven sync ("last-write-wins on `updated_at` vs. file mtime",
   "delta under 5 seconds → conflict").** `codex_service::compile.rs:220-224`
   updates `codex_article` without touching `updated_at`, so a recompiled
   article would never re-export. And a timestamp delta cannot distinguish a
   stale file from a divergently-edited one. Replaced by a stored content-hash
   merge base.

## Decisions locked during design review

- **Scope: full bidirectional.** Outbound + inbound + conflict + soft-delete,
  in one tranche (14 PRs).
- **Sync is content-hash based, not timestamp based.** A stored `synced_hash`
  per record is the merge base; no clock is consulted.
- **Layout mirrors ownership.** Two roots, `campaigns/` and `collections/`.
- **One file per record**, with an HTML-comment-fenced compiler-owned block.
- **Identity is the frontmatter `id`.** Paths are derived, never authoritative.
- **A type-folder move reconciles back** and raises a `vault_type_mismatch`
  lint finding. No cross-table retype.
- **An id-less file in a managed folder creates a record** directly, with
  strict path gating.
- **Ports for storage, watching, outbound, and record access.** The engine is
  a new crate that depends on none of them concretely.
- **No `is_gm_only`, no `vault_include_gm_only`.** Both deferred to Phase 3.
- **`yaml_serde` + `notify`.** Both verified green against `cargo deny check`.

## Vault layout

Each record maps to exactly one key, derivable from its owning edge.

```text
<vault_root>/
  campaigns/
    shadows-of-valdris/
      sessions/
        001-the-awakening.md
      entities/
        npc/seraphina-aldric.md
        location/the-iron-tower.md
  collections/
    dnd-5e-core/
      entities/
        creature/goblin.md
      rules/
        grappling.md
```

Everything outside `campaigns/*/…` and `collections/*/…` is **unmanaged and
ignored** — the vault root, `.obsidian/`, and `*.conflict.*.md`. That exclusion
applies on the **watcher path as well as reconcile**: a `.conflict.*.md` file
has no `id` and sits in a managed folder, so without it the file would fall
straight into the id-less-create path and spawn a duplicate record.

**Scope folders take the same slug + id-suffix rule as files.** `campaign.name`
has no unique index (`001_base_schema.surql:9-13`), so two campaigns can share a
name and would otherwise collide at the folder level. Renaming a campaign or
collection re-keys every file beneath it: the drain writes the new tree and
deletes the old under a single guard set. Inbound, the folder slug is **never**
parsed back into a scope — the id-less create path resolves its owning campaign
or collection from the reconcile scan's `slug → scope id` map, and ignores the
file if that lookup is ambiguous or absent.

## Identity

**The frontmatter `id` is the sole identity.** Reconcile does not compute an
expected slug and look for it. It scans the vault, parses frontmatter, and
builds an `id → key` map. Everything matches on `id`.

This follows from three schema facts: filenames derive from `name`; `name` is
not unique (two NPCs called "Guard" collide on `guard.md`); and `name` is
mutable, so a rename would orphan a path-keyed file.

Consequences:

- **Renaming a file in Obsidian is a cosmetic no-op.** The record is found by
  `id` regardless of filename; reconcile does not rename it back. (A _type
  folder_ move is different — the folder carries meaning, the filename does
  not.)
- **Slug collisions take a deterministic `id`-derived suffix:** `guard.md`,
  `guard-4f2a1c.md`.
- **A `Remove` event never directly soft-deletes.** See "Inbound".
- **A `Create` event bearing a known `id` is a relocation**, not a new record.

## File format

Frontmatter is **YAML with a shared namespace** — Obsidian ascribes meaning to
`aliases` and `title` and builds its own link graph from them. These are not
private serialisation keys.

`wikilink/mod.rs:74` parses `[[name]]` and resolves it against entity `name`,
case-insensitively. Files are named by slug. **Without `aliases`, every
`[[wikilink]]` in a compiled article renders as a broken link in Obsidian.**
Obsidian also matches aliases case-insensitively, so the two resolvers agree.

**All string scalars are emitted quoted, unconditionally.** An entity named
`Vex: The Unbound`, or one starting with `[`, otherwise produces invalid or
misparsed YAML.

### Entity — `campaigns/<slug>/entities/npc/seraphina-aldric.md`

```text
---
id: "npc:abc123"
name: "Seraphina Aldric"
title: "Seraphina Aldric"
aliases: ["Seraphina Aldric"]
type: "npc"
campaign: "Shadows of Valdris"
created_at: "2026-05-28T14:00:00Z"
updated_at: "2026-07-09T18:32:00Z"
---

## Summary

Half-elven archivist of the Iron Tower.

<!-- chronacle:codex-article start -- compiled; edits are not applied -->
Seraphina is the half-elven archivist of the [[The Iron Tower]]...
<!-- chronacle:codex-article end -->

## Notes

GM notes here.
```

`summary` is GM-owned — B1 froze it as byte-for-byte unchanged by compilation —
so it is an editable section, not fenced. The fence covers only `codex_article`.

### Rule entry — `collections/<slug>/rules/grappling.md`

Frontmatter carries `title`, `aliases`, `category`, and `page_refs` / `sources`
as read-only provenance. Fenced `body`; editable `## Notes`.

### Session — `campaigns/<slug>/sessions/001-the-awakening.md`

Frontmatter carries `session_number`, `title`, `date_played`, `aliases`. The
numeric filename keeps `sessions/` chronologically sorted while `title` drives
Obsidian's display and linking. **No fence** — a session has only `notes`, so
the whole body is GM-owned.

### Body grammar

Outbound renders the body **from the record**, so any GM prose the parser does
not recognise would be destroyed on the next write — and the next write can be
triggered by a compile the GM never asked for. The grammar is therefore
**lossless by construction**:

- The fenced block maps to `codex_article` (or `rule_entry.body`).
- A leading `## Summary` section, if present, maps to `summary`.
- **Everything else in the body maps to `notes`, verbatim** — including
  unrecognised headings, prose above the first heading, and a deleted
  `## Notes` heading.

So `## Notes` is a rendering convention, not a parsing requirement. A GM who
adds `## Ideas` keeps it: it round-trips through `notes`. Nothing outside the
fence is ever dropped. This is deliberately coarser than heading-scoped field
mapping, which is the single most likely source of D1a/D5a rework.

### Ownership

| Field / key                                 | Owner    | Inbound                           |
| ------------------------------------------- | -------- | --------------------------------- |
| `notes` (all non-fence, non-summary body)   | GM       | applied                           |
| `summary`, session body                     | GM       | applied                           |
| `name` / `title`                            | GM       | applied; triggers outbound re-key |
| `codex_article`, `rule_entry.body` (fenced) | compiler | **ignored**                       |
| `page_refs`, `sources`, `id`, `type`        | compiler | **ignored**                       |
| `aliases`, `campaign`, `collection`         | derived  | **ignored** (re-rendered)         |
| `created_at`, `updated_at`                  | database | **ignored**                       |

An `aliases` edit is plausible in an Obsidian workflow, so it is worth being
explicit: it is regenerated from `name` on every outbound write and never read
back.

**Fence and body comparison is normalized** — trim, CRLF→LF — before deciding
anything differs. A byte-exact compare manufactures a conflict every time an
editor appends a trailing newline.

## Architecture

The engine is backend-agnostic: markdown, frontmatter, key mapping, conflict
resolution, and reconcile operate on keys and content, not on `std::fs`.

```text
chronacle-core          traits + DTOs
  VaultStore            read/write/delete/list/metadata, keyed
  VaultWatcher          yields change events (optional per backend)
  VaultOutbound         enqueue(VaultRef) — one method
  VaultRecordStore      record access port
  VaultRef, VaultEvent, VaultRecord

chronacle-vault  ← NEW  the engine. core + yaml_serde (+ serde, tokio,
                        thiserror). No providers, no domain, no extraction,
                        no filesystem crate.
  VaultSyncService      reconcile · drain · conflict resolution
  frontmatter           yaml_serde render/parse, always-quote
  markdown              fence render/extract, normalized compare
  keys                  record ↔ key mapping, slug + collision suffix

chronacle-providers     concrete backends
  LocalFsVaultStore     tokio::fs
  NotifyWatcher         notify
  (S3VaultStore, WebDavVaultStore, PollingWatcher — later, no engine change)

chronacle-domain        SurrealVaultRecordStore — SurrealQL impl
apps/desktop            composition root: constructs, wires, spawns drain task
```

`chronacle-extraction` (the codex compiler) depends only on `chronacle-core`
for the one-method `VaultOutbound`, so the compiler never learns what a file
is. Nothing depends back on `chronacle-vault` except the app. `crates/*` is
already globbed into `workspace.members`; no root `Cargo.toml` change.

`VaultRecord` is a three-variant enum (`Entity` / `Session` / `RuleEntry`)
rather than five parallel method families, to keep `VaultRecordStore` from
sprawling as the codex grows.

**`VaultStore` is key-addressed, not path-addressed.** S3 has no `rename` and
no directories, so the trait exposes `write(key, content)` / `delete(key)` /
`list(prefix)` / `metadata(key) -> { mtime }`. A rename is
`write(new) + delete(old)` — exactly what the slug-change path already does.
There is no `rename()` in the trait.

**`VaultWatcher` is optional.** The service takes
`Option<Arc<dyn VaultWatcher>>`. `None` means reconcile-only. This is both the
mode a remote backend runs in and the honest degraded mode when a filesystem
watcher fails to initialise (network mounts, `inotify` limits) — rather than
the app refusing to sync.

### Why remote backends work

Reconcile is the **correctness guarantee**; the watcher is only a **latency
optimisation**. A backend with no change-notification mechanism is therefore
still correct: it ships a `PollingWatcher` or `None` plus "Sync now". Had the
design triggered outbound writes purely from service call sites with no
reconcile, remote backends would have been unimplementable without inventing a
change feed.

## Outbound

Services call `enqueue(VaultRef)` after a successful write — fire-and-forget
onto an mpsc channel. A background **drain task** coalesces repeats (compiling
200 entities enqueues 200 refs; the drain writes each key once), then per ref:

1. Render the file from the record.
2. Record a `pending_write` guard: `(key, hash(content))`, where `hash` is a
   64-bit `std::hash::DefaultHasher` digest. This is a loop guard, not a
   security primitive — a cryptographic hash would need a new approved crate
   for no benefit.
3. `VaultStore::write`.

**Both halves of that handshake live inside the drain task** — which is why the
trigger does not belong in the services.

Guard semantics matter, because one `write()` commonly emits several events
(`Create` + `Modify`, or `Modify(Data)` + `Modify(Metadata)`), and a guard whose
event never arrives would otherwise live forever:

- The inbound handler **re-reads the key and hashes its content**, then drops
  the event if the hash matches a live guard. It does not match on key alone.
- Guards are **not consumed on first match** — a single write's trailing events
  must all be dropped. They expire on a TTL (30s), or when the key's content
  hash is observed to differ.
- Because matching is content-based, a stale guard cannot mask a genuine later
  edit: different content, different hash.
- **The frontmatter `id` write-back on inbound create is itself a file write**
  and must be issued under a guard, or it re-triggers the watcher as an inbound
  modify.

Producers to wire: `entity_service`, `session_service`,
`codex_service::compile_collection`, the rules pipeline, and the accepted-
`codex_proposal` path. A design that hooked only the obvious CRUD paths would
silently miss the compiler — the content the vault most needs to mirror.

An inbound change to `name` re-keys the file: the drain performs
`write(new) + delete(old)` under a guard covering **both** keys.

### Reconcile — bidirectional, hash-based

**Reconcile is not timestamp-based, and it is not outbound-only.** Both
properties are load-bearing; the reasons are worth stating because the obvious
design fails.

`updated_at` cannot drive sync. `codex_service::compile.rs:220-224` writes
`codex_article`, `codex_compiled_at`, `codex_stale`, and `codex_sources` — and
never touches `updated_at`. A reconcile comparing `mtime` against `updated_at`
would therefore never re-export a recompiled article, leaving the vault
permanently stale in exactly the producer that matters most.

Timestamps also cannot distinguish _"the file is a stale copy of the DB"_ from
_"the file holds unapplied divergent edits."_ That distinction needs a **base**
to compare both sides against.

So the engine stores a **last-synced content hash per record** in a
`vault_sync_state` table (`record`, `key`, `synced_hash`, `synced_at`), and
sync becomes an ordinary three-way merge:

| `db_hash` vs base | `file_hash` vs base | Action                    |
| ----------------- | ------------------- | ------------------------- |
| same              | same                | no-op                     |
| changed           | same                | **export** (outbound)     |
| same              | changed             | **apply** (inbound)       |
| changed           | changed             | **conflict**              |
| —                 | file absent         | soft-delete (see Inbound) |
| no base           | file absent         | export (first write)      |

`db_hash` is the hash of the record rendered to markdown; `file_hash` is the
hash of the file's normalized content. Both sides are compared to
`synced_hash`. **No clock is consulted.** `mtime` survives only as an
optimisation — a file whose `mtime` is unchanged since `synced_at` need not be
read and hashed.

This makes reconcile inherently **bidirectional**: it applies inbound edits made
while the app was closed, and it is the _only_ inbound path for a backend
running with `watcher = None`. Reconcile runs on startup, on `vault_sync_path`
change, and on explicit "Sync now".

**Reconcile skips soft-deleted records.** Queries use the `vault_deleted != true`
form — never `= false`. A `DEFINE FIELD … DEFAULT false` migration does not
backfill existing rows, so records written before `003_vault_sync.surql` carry
no `vault_deleted` value at all and would silently fail a `= false` filter.
(This repo has been bitten by a silently-wrong SurrealDB filter before; see the
`count()` + `GROUP ALL` case.)

A dropped `enqueue()` degrades to _"the file updates on next reconcile"_ —
never to _"the file is permanently wrong."_ That claim is now true, because
reconcile compares content, not timestamps that a producer might forget to
bump.

## Inbound

**Watcher events are hints, not facts.** `notify-types` 2.1.0 emits
`RenameMode::Both` only "when both source and target are known"; otherwise a
rename decomposes into `Remove(old)` + `Create(new)`, or a bare
`RenameMode::Any`. Editors additionally save atomically by writing a temp file
and renaming over the target, so `Remove` is routinely emitted for files that
still exist.

Every event therefore re-reads the affected key and re-derives truth from
frontmatter.

| Event                               | Action                                                                                                                                                                                                            |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Modify**, known `id`              | Apply GM-owned fields only. Fenced content, `page_refs`, `sources`, `id`, `type` ignored.                                                                                                                         |
| **Modify**, no `id`, managed folder | id-less create path.                                                                                                                                                                                              |
| **Remove**                          | **Rescan the vault for that `id` first.** If any key still carries it, this was a rename or relocation — update nothing. Only if the `id` is absent everywhere: `vault_deleted = TRUE` + restore-or-confirm card. |
| **Create**, known `id`              | Relocation. If the destination is a different _type_ folder: leave the record, reconcile the file back to its canonical key, raise a `vault_type_mismatch` lint finding.                                          |
| **Create**, no `id`, managed folder | Create the record, embed it, write the `id` back into frontmatter.                                                                                                                                                |
| Anything unmanaged                  | Ignored.                                                                                                                                                                                                          |

Inbound record creation needs an embedding and a wikilink re-sync. Both happen
inside `SurrealVaultRecordStore::create`, which lives in `chronacle-domain` and
delegates to `entity_service` — which lives in **`chronacle-extraction`**, not
in domain. `chronacle-domain/Cargo.toml:14` already depends on
`chronacle-extraction`, so no cycle arises. `SurrealVaultRecordStore` is
constructed with `Arc<dyn EmbeddingProvider>` injected, and calls extraction's
entity-create plus the wikilink back-scan (`wikilink::backfill`). The engine
stays ignorant of embeddings, SurrealQL, and files simultaneously.

### Soft delete

`vault_deleted` is added to the **eight entity tables and `session`** (not to a
nonexistent `entity` table), in a new `003_vault_sync.surql`, alongside the
`vault_sync_state` table. Migrations are `DEFINE`-only and re-run every boot, so
this is `DEFINE FIELD OVERWRITE vault_deleted ON TABLE <t> TYPE bool DEFAULT false`
×9 — never a `REMOVE`, which once wiped every relationship edge on restart.

`DEFAULT false` **does not backfill existing rows.** Records created before this
migration carry no `vault_deleted` value, so a `WHERE vault_deleted = false`
filter silently omits every pre-existing entity. All queries use
`vault_deleted != true`. D2b ships a test asserting a row created before the
migration is treated as not-deleted.

`vault_sync_state` is `SCHEMAFULL` with typed fields (`record`, `key`,
`synced_hash`, `synced_at`) — no `FLEXIBLE` object, no `serde_json::Value`
binding on writes. The same applies to the `vault_type_mismatch` payload written
into `lint_finding.payload`, which **is** `FLEXIBLE` (`002_wiki_layer.surql:33`):
it is written from a typed struct, following the existing lint-finding pattern.

"Restore" flips the flag and re-exports. "Confirm delete" hard-deletes the
record.

**A soft-deleted record is excluded everywhere except the Maintenance view's
pending-deletions list** — entity lists, RAG retrieval, wikilink resolution,
codex compile inputs, and lint passes all filter it out. Otherwise a "deleted"
entity keeps answering questions and getting recompiled while its file stays
absent, and the app and vault disagree about what exists.

Every one of those read paths needs the `vault_deleted != true` filter, across
`chronacle-extraction` (entity, codex, lint), `chronacle-retrieval`, and
`chronacle-domain`. This is a cross-crate change and is budgeted into D5c, not
a footnote to it.

A `codex_proposal` targeting a soft-deleted record is left pending; accepting it
is rejected with an error rather than resurrecting the record implicitly.

## Conflict

**A conflict is `db_hash ≠ base ∧ file_hash ≠ base`.** No time window is
involved. ADR-008's "delta under 5 seconds" rule is superseded: it both
under-detects (a file edited offline, then a record edited in-app an hour
later, silently loses the file's edit) and over-detects (every outbound write
leaves `mtime ≈ updated_at`, so a GM edit three seconds after an export
manufactures a spurious conflict).

On conflict: the file's version is copied to `<slug>.conflict.<ts>.md`, the DB
version is written to the canonical key, `synced_hash` is updated to the DB
render, and a conflict card surfaces in the Maintenance view. Nothing is
discarded.

**The conflict file has its `id` demoted to `conflict_of:` and its `aliases` /
`title` stripped.** A verbatim copy would carry a duplicate `id` — poisoning
the `id → key` map the entire identity model rests on — and a duplicate alias,
hijacking Obsidian's wikilink resolution. Obsidian indexes and links every `.md`
it sees; `.conflict.*.md` is a document it will surface, not an inert backup.

## Known limitation: alias collisions

Two entities named "Guard" produce `guard.md` and `guard-4f2a1c.md`, both
carrying `aliases: ["Guard"]`. `[[Guard]]` is ambiguous and Obsidian silently
picks one. Our own resolver has the identical ambiguity (`.find()` on first
case-insensitive name match, `wikilink/mod.rs:112`), so the vault faithfully
reproduces a limitation that already exists in the app. Documented, not solved
here.

## Dependencies

Both verified against the live advisory DB with a temporary manifest edit;
`cargo deny check advisories licenses bans` → `advisories ok, bans ok,
licenses ok`. Manifests reverted.

| Crate        | Version | Where                 | Notes                                                                                                                                                 |
| ------------ | ------- | --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `yaml_serde` | 0.10.4  | `chronacle-vault`     | `github.com/yaml/yaml-serde` (the YAML org). MIT OR Apache-2.0. Backed by `libyaml-rs` 0.3.0, no advisory. `rust-version` 1.82 < workspace MSRV 1.95. |
| `notify`     | 8.2.0   | `chronacle-providers` | No advisory. OS-native APIs.                                                                                                                          |

ADR-008 and the architecture doc's **Crate & Tool Summary** name `serde_yaml`.
Both are amended on D0. `serde_yaml` 0.9.34 is archived (its version string
reads `+deprecated`) and sits on `unsafe-libyaml`; `serde_yml`, the fork with
the most downloads, carries **RUSTSEC-2025-0068** (unsound + unmaintained,
serializer segfault) and would fail our `unsound = "workspace"` gate as a
direct dependency.

`chronacle-vault` gains a row in the **Internal Workspace Crates** table.
Internal crates do not require an ADR.

## Test strategy (TDD, per repo conventions)

The ports make the interesting states reachable — clock skew, I/O failure
mid-write, a `Remove` for a file that still exists — all of which are
_unreachable_ through a real filesystem and trivially constructible through a
mock.

- **`chronacle-vault` engine — pure unit tests, no DB, no disk.**
  `MockVaultStore` + `MockVaultRecordStore` (`mockall`). The three-way decision
  table is exhaustively table-driven over `(base, db_hash, file_hash)` — every
  row of it, including the crash-recovery `db_hash == file_hash ≠ base` case,
  reachable without a clock or a disk. The `Remove`-but-file-still-exists
  decomposition is a scripted event sequence; I/O failure mid-write is
  `MockVaultStore` returning `Err`.
- **`LocalFsVaultStore` — integration tests** against `tempfile::TempDir`. The
  only place real filesystem semantics are exercised; it is a thin adapter.
- **`NotifyWatcher` — integration tests** with a real temp dir, asserting
  debounce coalescing. Deliberately narrow; the engine never depends on watcher
  fidelity.
- **`SurrealVaultRecordStore` — integration tests** on `mem::Db`, including the
  `vault_deleted` migration running twice (idempotency).
- **Frontend** — Vitest + `@testing-library/svelte` for the settings panel,
  restore-or-confirm card, and conflict card.
- **Acceptance (ADR-011)** — `.feature` scenarios ship with every user-visible
  PR.

## BDD scenarios (acceptance criteria)

Transferred verbatim into `apps/desktop/tests/e2e/features/` during planning.

**D3 — reconcile & settings**

- Given a campaign with entities and sessions and no vault configured, when the
  GM sets a vault path, then a full reconcile writes one `.md` per record under
  `campaigns/<slug>/`, and each entity file carries `aliases` matching its name.
- Given a configured vault and no changes, when the GM clicks "Sync now", then
  no file contents change.
- Given a record with `vault_deleted = TRUE`, when reconcile runs, then no file
  is written for it.
- Given a collection subscribed to two campaigns, when reconcile runs, then its
  entities appear exactly once, under `collections/<slug>/`.

**D4 — outbound**

- Given a configured vault, when the GM edits an entity's notes in Chronacle,
  then the corresponding `.md` body updates and no inbound event is applied.
- Given a compiled collection, when the GM compiles it again, then each changed
  entity's file is written exactly once, not once per enqueue.
- Given an entity renamed in Chronacle, then the old key is deleted and a new
  key written, both under one `pending_write` guard.

**D5 — inbound**

- Given an entity file, when the GM edits its `## Notes` section in the vault,
  then `notes` updates in the database and `codex_article` is unchanged.
- Given an entity file, when the GM edits text inside the codex-article fence,
  then `codex_article` is unchanged in the database.
- Given an entity file, when the GM renames the file within the same folder,
  then the record is unchanged and no file is renamed back.
- Given an entity file, when the GM moves it to a different type folder, then
  `entity_type` is unchanged, the file is reconciled back to its canonical
  folder, and a `vault_type_mismatch` lint finding appears.
- Given a new `.md` file with no frontmatter placed in
  `campaigns/<slug>/entities/npc/`, then an `npc` record is created, embedded,
  and the assigned `id` is written back into the file's frontmatter.
- Given a new `.md` file placed at the vault root, then no record is created.
- Given an entity file, when the GM deletes it, then `vault_deleted` becomes
  `TRUE` and a restore-or-confirm card appears.
- Given an entity file moved by an editor's atomic save (`Remove` then
  `Create` with the same `id`), then `vault_deleted` remains `FALSE`.
- Given a soft-deleted record, when the GM chooses "Restore", then
  `vault_deleted` becomes `FALSE` and the file is re-exported.
- Given a soft-deleted entity, when the GM asks the agent a question it would
  have answered, then it is absent from retrieval, from wikilink resolution, and
  from compile inputs.
- Given an entity row created before `003_vault_sync.surql` ran, when reconcile
  runs, then it is treated as not-deleted and exported.

**D5a — inbound reconcile (no watcher)**

- Given a vault file edited while Chronacle was closed, when the app starts and
  reconcile runs, then the edit is applied to `notes` — with no watcher event
  ever fired.
- Given a vault configured with `watcher = None`, when the GM clicks "Sync now"
  after editing a file, then the edit is applied.
- Given a collection recompiled such that `codex_article` changed but
  `updated_at` did not, when reconcile runs, then the file is re-exported.
- Given a `.conflict.<ts>.md` file modified in a managed folder, when the
  watcher fires, then no record is created from it.

**D6 — conflict**

- Given a record modified in Chronacle and its file modified in the vault, both
  since the last sync, then a `<slug>.conflict.<ts>.md` is written containing
  the file's version, the canonical key holds the database version, and a
  conflict card appears — regardless of how far apart the two edits occurred.
- Given a record and its file whose contents are identical but whose
  `synced_hash` is stale (simulating a crash after write), when reconcile runs,
  then no conflict is raised and the base is adopted.
- Given a conflict file, then its frontmatter carries `conflict_of` and no
  `id`, `aliases`, or `title`.
- Given a conflict file present in the vault, when reconcile runs, then it is
  ignored and no record is created from it.
- Given a file whose body differs from the database only by a trailing newline
  or CRLF line endings, then no conflict is raised.

## Delivery plan — PR slicing

Ground rules carried from the A/B/C series: every PR is a feature branch that
does **not** track main (`git checkout -b <branch> --no-track`),
subagent-driven, ≤ ~800 lines, tests ship in the same PR, TDD-ordered (failing
tests first), green CI before merge.

**Outbound lands and goes green before the watcher exists**, so the hard
inbound work sits on a proven base rather than being debugged simultaneously
with export.

| PR  | Branch                    | Content                                                                                                                                             |
| --- | ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| D0  | `chore/d0-vault-crate`    | New crate + core traits/DTOs; deps; ADR-008 amendment; architecture tables; AGENTS.md setting-key fix                                               |
| D1a | `feat/d1a-frontmatter`    | Frontmatter render/parse (always-quote, `aliases`/`title`), fence render/extract, body grammar, normalized compare                                  |
| D1b | `feat/d1b-key-mapping`    | Record ↔ key mapping, slug + collision suffix (files _and_ scope folders), managed-folder gating, `id → key` scan                                   |
| D2a | `feat/d2a-fs-store`       | `LocalFsVaultStore` + TempDir integration tests                                                                                                     |
| D2b | `feat/d2b-record-store`   | `SurrealVaultRecordStore`; `003_vault_sync.surql` (`vault_deleted` ×9 + `vault_sync_state`); pre-migration-row test                                 |
| D3a | `feat/d3a-reconcile`      | Three-way reconcile, **export direction only** (apply/conflict → no-op + log); `vault_sync_state` base; `set_vault_sync_path` + `sync_now` commands |
| D3b | `feat/d3b-settings-ui`    | Settings UI: vault path picker, "Sync now", progress events                                                                                         |
| D4a | `feat/d4a-outbound-queue` | `VaultOutbound` + drain task + `pending_write` guard (content-hash, TTL) + coalescing                                                               |
| D4b | `feat/d4b-wire-producers` | Wire producers: `entity_service`, `codex_service`, rules pipeline, proposal-accept, `session_service`                                               |
| D5a | `feat/d5a-inbound`        | `NotifyWatcher` + debounce; **reconcile apply direction**; inbound modify → GM-owned fields; loop-guard verification                                |
| D5b | `feat/d5b-inbound-create` | id-less create (guarded `id` write-back) + relocation; `vault_type_mismatch` lint finding                                                           |
| D5c | `feat/d5c-soft-delete`    | `Remove` → id rescan → `vault_deleted`; `!= true` filters across extraction/retrieval/domain; restore-or-confirm UI                                 |
| D6  | `feat/d6-conflict`        | Conflict materialisation: `.conflict.<ts>.md` with `id` demoted, `aliases`/`title` stripped; conflict card                                          |
| D7  | `docs/d7-user-guide`      | GM-facing vault guide; ADR-008 → Accepted                                                                                                           |

Dependency chain: D0 → D1a → D1b → {D2a, D2b} → D3a → D3b → D4a → D4b → D5a →
{D5b, D5c} → D6 → D7. D2a and D2b are independent of each other; D5b and D5c
are independent of each other.

Two sequencing constraints, both about shipping a safe intermediate state:

- **D3a implements the full three-way decision but acts only on `export`.** The
  `apply` and `conflict` branches log and no-op. Outbound lands, goes green, and
  is releasable before any inbound path exists — while the decision table it will
  later depend on is already under test.
- **D5a therefore never applies a file over divergent DB state.** It turns on the
  `apply` branch, which the base hash already protects; a conflict is skipped and
  left for D6 to materialise into a `.conflict` file and a card. A release cut
  between D5a and D6 loses no data — it merely defers conflicts.

D5a is the riskiest PR (loop prevention), which is why D4a's `pending_write`
guard is proven by outbound tests before a watcher can trip it. D4a and D4b are
split because wiring five producers across three crates would otherwise push the
slice past the ~800-line budget.

## Risks & tradeoffs

- **Loop prevention is the sharpest edge.** A guard miss produces a
  write → watch → write cycle. Mitigated by proving the guard in D4a's outbound
  tests before D5a introduces a watcher, and by content-hashing the guard so a
  no-op rewrite is idempotent rather than self-triggering.
- **`vault_sync_state` can drift** if the app is killed between a
  `VaultStore::write` and the `synced_hash` update. The failure is benign and
  self-correcting: the next reconcile sees `db_hash ≠ base ∧ file_hash ≠ base`
  and would raise a conflict whose two sides are identical. Reconcile therefore
  short-circuits `db_hash == file_hash` to "no-op, adopt as base" **before**
  evaluating the conflict branch. D3a tests this crash-recovery path explicitly.
- **Debounce tuning is platform-dependent.** 100 ms per ADR-008. Editors that
  write-then-rename may need coalescing across the pair; the `Remove` id-rescan
  makes this safe regardless.
- **Inbound create on the watcher path costs an embedding call.** A GM pasting
  fifty notes into the vault triggers fifty embeddings. Accepted for now; the
  drain task serialises them, so it degrades to slow rather than to
  thundering-herd.
- **`yaml_serde` is not a dtolnay crate.** It is the YAML organisation's
  successor, actively maintained, but less battle-tested than `serde_yaml` was.
  The frontmatter surface is a flat map we control, which bounds the exposure.
- **`vault_deleted` on nine tables** is nine near-identical `DEFINE FIELD`
  lines. Consistent with how `codex_article` ×8 already works.

## Documentation plan

- **D0** — ADR-008 amended: data model, layout, ports, `yaml_serde`,
  `is_gm_only` removal. Architecture "Crate & Tool Summary" and "Internal
  Workspace Crates" tables updated.
- **D7** — `docs/user-guide.md` gains "Syncing your codex to Obsidian":
  choosing a vault, what's editable and what isn't, what a conflict card means,
  restore-vs-confirm on delete.
- **AGENTS.md** — the `setting` keys list currently names
  `vault_include_gm_only`. That key is not shipping; the line is corrected on
  D0 so the docs stop advertising a setting that does not exist.

## Open questions (remaining)

None blocking. Deferred by decision:

1. **Cross-table entity retype** — no Chronacle-side operation exists either.
   When one is built, the vault's `vault_type_mismatch` finding becomes its
   natural entry point.
2. **Player-safe export** — Phase 3, gated on AI-detected passage-level
   GM-secret flags. `vault_include_gm_only` returns with it.
3. **Remote backends** (S3, WebDAV) — the ports exist; no engine change needed.
   Not built here.

## Resolved during design review

- Scope, layout, file shape, identity model, type-move behaviour, inbound
  create path, port boundaries, `is_gm_only` omission, YAML crate: see
  "Decisions locked".
- `aliases` / `title` frontmatter and unconditional quoting: added on review
  after confirming `wikilink/mod.rs` resolves `[[name]]` against entity `name`,
  not slug — without which every compiled wikilink renders broken in Obsidian.
- `.conflict.*.md` `id`-demotion and alias-stripping: follows from Obsidian
  treating frontmatter as a shared namespace it indexes, not inert text.
- `chronacle-vault` as a separate crate with a `VaultRecordStore` port: chosen
  so a future HTTP/S3/WebDAV backend needs no engine change.
