# Vault Inbound Sync + Filesystem Watcher (Tranche 5) — Design

**Date:** 2026-07-12
**Status:** Approved for planning
**Predecessor:** Tranche 4 (D-series, one-way Markdown vault export), merged at `d952f13`. Handover: `docs/superpowers/tranche-5-handover.md`.

## Goal

Turn on the inbound direction of vault sync: GM edits in the Obsidian vault flow
back into the database, deletions soft-delete, divergent edits surface as
conflicts the GM resolves in the vault — and a filesystem watcher makes all of
it near-live. Fix the two landmines (L1, L2) that gate `SoftDelete` safety.

## Decisions (resolved in brainstorming)

| # | Question | Decision |
| --- | --- | --- |
| 1 | Conflict UX | Sidecar `<stem>.conflict.md` + a conflict **list** (not just a count) in the Vault Sync settings panel, plus an in-place banner on affected records. No in-app resolution UI this tranche — resolution happens in the vault. |
| 2 | Conflict termination | **Sidecar deletion = resolved.** The record freezes (no apply, no export, no base update) while the sidecar exists; deleting the sidecar tells the next reconcile "apply my file". Requires a per-record `conflict` flag on `vault_sync_state`. |
| 3 | Fence edits | **Revert on apply.** GM-owned parts go to the DB, then the canonical render is re-exported over the file. The fence marker already says "compiled; edits are not applied". |
| 4 | Apply scope | **Body only.** Entity `summary` + `notes`, session `notes`, rule-entry `notes`. Frontmatter is compiler-owned like the fence; edits to it are reverted by the re-export. Entity renames happen in the app; renaming a *file* stays cosmetic (index-wins, unchanged). |
| 5 | L2 fix | **Fresh baseline on switch.** Changing `vault_sync_path` clears all `vault_sync_state` rows, reconciles against the new dir, and persists the setting only after the reconcile succeeds. |
| 6 | Inbound flow | **Watcher triggers reconcile.** Reconcile is the only code path that materializes `Apply`/`Conflict`/`SoftDelete`; the watcher is a debounced trigger. Mirrors the outbound design (reconcile = truth, queue/watcher = latency). |
| 7 | Extra scope | Include I1 (reconcile after bulk extraction) and the remaining D-series minors. |

## Architecture

```
notify (fs events)
   │
NotifyWatcher (chronacle-providers)          ← new
   │  · maps paths → vault keys (.md only)
   │  · debounce ~2s quiet window (tokio, hand-rolled — no new crates)
   │  · drops events matching PendingWrites (our own writes)
   │  · emits VaultEvent over the existing subscribe() port
   ▼
watcher task (src-tauri, spawned beside drain_loop)
   │  · coalesces: single in-flight reconcile + dirty flag
   ▼
VaultSyncService::reconcile()                ← materializes inbound
   │  Apply     → apply GM parts via VaultRecordStore → re-export canonical → set base
   │  Conflict  → sidecar lifecycle (below)
   │  SoftDelete→ vault_deleted = true, clear base
   ▼
chronacle-domain (SurrealVaultRecordStore → entity/session services)
```

Reconcile stays the correctness guarantee: a missed or dropped watcher event
degrades to "handled on the next reconcile", never to wrong data.

### Apply (SyncAction::Apply)

1. Read the file; `frontmatter::parse` for identity sanity, `split_body` for regions.
2. Write GM-owned parts to the DB via a new `VaultRecordStore::apply_gm_parts(vref, parts)`.
   The `chronacle-domain` impl routes through `entity_service` / `session_service` /
   rule-entry service so validation, wikilink extraction, and embedding updates
   stay consistent. `chronacle-vault` stays fs- and DB-free.
3. Re-render the updated record and write the canonical render back to the file
   (arming `PendingWrites` first), then `set_synced_hash`. Fence and frontmatter
   edits are thereby reverted in the same pass; the record settles to `NoOp`.

A file whose frontmatter fails to parse (no id, broken YAML) is counted in a new
`ReconcileReport.invalid` bucket and logged; it is never applied and never
overwritten. Unmanaged files (an id no record claims, or no frontmatter) are
ignored, as today.

### Conflict lifecycle (SyncAction::Conflict)

State: a `conflict: bool` flag (default false) on `vault_sync_state`, plus the
sidecar file `<stem>.conflict.md` next to the GM's file, containing the DB's
canonical render. Sidecars are compiler-owned.

| Observed state | Action |
| --- | --- |
| `Conflict`, flag unset | Write sidecar (armed in `PendingWrites`), set flag. Freeze: no apply, no export, no base update. |
| `Conflict`, flag set, sidecar present | Stay frozen. Refresh the sidecar if the DB render changed (keeps it from going stale). |
| `Conflict`, flag set, sidecar **gone** | GM resolved: run the Apply path on the GM's file (apply GM parts, re-export canonical, set base), clear flag. |
| Non-conflict action, flag set | Conflict evaporated (e.g. GM reverted the file): delete the sidecar, clear flag, proceed with the normal action. |

Known, accepted trade-off: a record in conflict stops exporting DB changes
until the GM resolves it. Because the `conflict` flag is persisted on
`vault_sync_state`, conflicts are queryable at any time — a new
`list_vault_conflicts()` IPC command returns each conflicted record's name,
kind, vault file key, and sidecar key. The settings panel renders these as a
list (badge count derives from its length), and the entity/session editor
shows a banner on affected records (see Frontend changes).

`*.conflict.md` keys are excluded from `VaultIndex::scan` and from reconcile's
record matching — the sidecar carries the same frontmatter `id` as the real
file and must never hijack the ref→key mapping.

The unmanaged-file conflict (`base = None`, file differs from DB) follows the
same lifecycle; the flag row is created with the key but without a base.

### SoftDelete (SyncAction::SoftDelete)

New `VaultRecordStore::soft_delete(vref)`: sets `vault_deleted = true` on the
record and clears the synced base. Read paths already filter
`vault_deleted != true` (schema `003_vault_sync.surql`). No undelete UI this
tranche; a soft-deleted record is invisible to the app and to future exports.

### L2 — fresh baseline on vault-path switch

In the `set_vault_path` command, when the new path differs from the stored one:

1. Build the new stores against the new path.
2. `VaultRecordStore::clear_all_synced()` (new method) — wipes every
   `vault_sync_state` row (bases, keys, conflict flags).
3. Run a full reconcile against the new dir. With no bases, `decide()` can only
   return `Export` / `AdoptBase` / `Conflict` — never `SoftDelete` — so a fresh
   dir is a clean first export and an identical restored vault adopts bases.
4. Persist the `vault_sync_path` setting **only after** the reconcile succeeds
   (also closes the carried D-minor about ordering). On failure the old path
   and old bases remain in force.

Stale sidecars left in the old directory are inert.

### L1 — IPC command surface

- `soft_delete_entity(id)` Tauri command routed through `entity_service`
  (sets `vault_deleted = true`). The existing `delete_entity` hard-delete
  remains for genuine destruction.
- **Orphan sweep (new reconcile step):** a soft-deleted (or hard-deleted)
  record disappears from `list_all`, so today its vault file and
  `vault_sync_state` row would linger forever — reconcile only iterates
  records. Reconcile gains a final pass over `vault_sync_state` rows whose
  record no longer syncs: delete the vault file (armed in `PendingWrites`)
  and clear the row. Never-clobber applies: the file is deleted only if its
  content hash still equals the base; if the GM edited it since, the row is
  cleared but the file is left in place as an unmanaged file. This pass is
  what makes in-app deletion propagate outbound. (Requires a new
  `VaultRecordStore` method to enumerate synced refs + keys.)
- `create_entity` gains optional `collection_id` (and `campaign_id` becomes
  optional accordingly) so collection-scoped entities can be created over IPC —
  `entity_service::create` already supports it.

### NotifyWatcher (chronacle-providers)

Implements the existing `VaultWatcher` port (`subscribe() → mpsc::Receiver<VaultEvent>`)
over `notify = "8"` (already approved, ADR-covered):

- Watches the vault root recursively; only `.md` paths become events
  (`Upsert`/`Remove`); non-`.md` and directory noise is dropped.
- Debounce: collect raw events until a ~2s quiet window elapses, then flush.
- Loop guard: for each `Upsert`, read + hash the file and drop the event when
  `PendingWrites::matches(key, hash)` — the 30s TTL and content-hash keying
  from D4a are kept as-is. `Remove` events are never guard-dropped.
- Watcher overflow / rescan-worthy errors map to `VaultEvent::Rescan`.
- The consumer task (in `src-tauri`, spawned beside `drain_loop`, respawned by
  `set_vault_path`) treats any surviving event batch as "trigger one
  reconcile", with single-in-flight + dirty-flag coalescing.

Reconcile's and the drain's writes all arm `PendingWrites` (reconcile's
currently do not — this tranche adds it) so inbound passes don't re-trigger
themselves; even when a self-write slips through, the resulting reconcile is a
`NoOp`, so the guard is an optimization, not a correctness requirement.

### I1 — post-bulk-extraction sync

After a bulk PDF extraction batch completes, trigger one reconcile (same
mechanism as the watcher's trigger). `persist_batch` keeps its `NoopOutbound`
to avoid queue flooding; the single trailing reconcile brings the vault
current.

## Schema changes (DEFINE-only, idempotent)

- `vault_sync_state.conflict: bool` (default false).

No other schema changes. `synced_hash` stays a per-record string; scoping is
handled behaviourally by the fresh-baseline rule.

## Frontend changes

All new UI in Svelte 5 runes.

- **Vault Sync settings panel:** a conflict *list* — one row per conflicted
  record showing name, kind, and the vault file + sidecar paths — fed by
  `list_vault_conflicts()`, with a badge count derived from it, plus the
  `invalid` count from the last reconcile. Each row carries a short inline
  hint: "Merge the two files in your vault, then delete the `.conflict.md`
  file — Chronacle applies your version on the next sync."
- **Record editors (entity/session):** when the open record is conflicted, a
  non-blocking banner: "This record has unsynced vault edits in conflict —
  resolve in your vault" with the sidecar filename.
- **UI hints at select places:** a one-line explainer next to the vault-path
  setting ("Changing the folder re-exports everything; nothing is deleted"),
  and a note near the fence-related behaviours in the settings panel help
  text ("Text inside the marked compiled block is overwritten by Chronacle").
- **Entity UI:** delete action calls `soft_delete_entity`.

## Documentation (ships in this tranche)

`docs/user-guide.md` gains a GM-facing "Your Vault" chapter, written for
non-technical readers (`user-guide-writer` agent):

- What vault sync is: your campaign as ordinary Markdown files, editable in
  Obsidian or any editor; changes flow both ways.
- What is yours vs Chronacle's in a file: Summary and Notes are yours; the
  marked "compiled" block and the metadata header are Chronacle's and get
  rewritten (with a worked example file).
- Conflicts: why they happen, what a `.conflict.md` file is, and the exact
  resolution walkthrough (compare, merge into your file, delete the sidecar).
- Deleting: removing a vault file hides the record in Chronacle (soft
  delete); what switching vault folders does and does not do.

## Error handling

- Per-record failures during reconcile are logged and counted (`failed`),
  never abort the run (unchanged).
- Unparseable vault files: counted as `invalid`, skipped, never overwritten.
- `set_vault_path` failure leaves the previous path and bases untouched.
- Watcher channel closure / notify errors: log, emit `Rescan`, and keep the
  app functional — reconcile remains manually triggerable from settings.

## Testing

- **Unit (`chronacle-vault`, mockall):** Apply writes GM parts only and
  re-exports; fence/frontmatter edits reverted; conflict lifecycle table above
  (all four rows); soft-delete never resurrects a file; sidecar keys excluded
  from index; invalid files skipped and counted; `clear_all_synced` +
  no-base decide never yields SoftDelete.
- **Providers integration (`TempDir`, real fs):** NotifyWatcher debounce,
  `.md` filtering, guard-drop of armed writes, Remove events, Rescan on
  overflow.
- **Tauri integration (`mem://` SurrealDB):** end-to-end apply round-trip
  through `SurrealVaultRecordStore` → services; soft-delete via reconcile;
  vault-path switch produces zero soft-deletes; `soft_delete_entity`,
  collection-scoped `create_entity`, and `list_vault_conflicts` commands.
- **Frontend (Vitest + testing-library):** conflict list renders rows from
  `list_vault_conflicts`; record-editor banner appears for a conflicted
  record and not otherwise.
- **Acceptance (`.feature`, ADR-011):** GM edits notes in vault → DB updates;
  GM edits inside the fence → reverted; both sides edited → sidecar appears,
  badge counts it; GM deletes sidecar → file version applied; GM deletes a
  vault file → record soft-deleted; switching vault folders deletes nothing.
- **D-series minors:** exact-output test for the frontmatter/body seam;
  Vitest for `VaultSyncSettings` error/reject path; preserve `io::ErrorKind`
  in `VaultStoreError::Io`.

## Out of scope

- In-app conflict *resolution* UI (conflicts are listed and flagged in-app,
  but merging happens in the vault).
- Inbound frontmatter fields (`name`, session metadata) — body only.
- Undelete / trash UI for soft-deleted records.
- Path-scoped merge bases (fresh-baseline chosen instead).
- Non-fs vault backends' change feeds (reconcile covers them).
