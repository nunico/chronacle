# Tranche 5 Handover — Inbound Vault Sync + Filesystem Watcher

**Status of tranche 4 (D-series, Markdown Vault Sync):** COMPLETE, merged to `main` (`d952f13`), PRs #19–#30. Chronacle now does **one-way export**: compile the codex → it appears as an Obsidian-compatible Markdown vault, live. Verified end-to-end including a real-app run in the Linux tauri-driver container.

Tranche 5 turns on the **inbound** direction (GM edits the vault → changes flow back into the DB) and adds the **filesystem watcher** that makes it live.

---

## 1. What already exists (do not rebuild)

The export tranche deliberately built the inbound machinery's scaffolding. Before writing anything, know what's already there:

| Piece | Location | State |
| --- | --- | --- |
| **`decide()`** — pure three-way merge | `crates/chronacle-vault/src/decide.rs` | Already returns `Apply` / `Conflict` / `SoftDelete` / `AdoptBase` / `Export` / `NoOp`. **No change needed** — the inbound actions are computed today, just not acted on. |
| **`reconcile()`** deferral point | `crates/chronacle-vault/src/reconcile.rs:114-121` | The `Apply \| Conflict \| SoftDelete` match arm currently only logs + increments `report.deferred_*`. **This is the exact turn-on point.** Replace the log with real materialization. |
| **`VaultWatcher` port** | `crates/chronacle-core/src/vault.rs:88` | `async fn subscribe(&self) -> tokio::sync::mpsc::Receiver<VaultEvent>`. `VaultEvent` enum at `:49`. Implement a concrete `NotifyWatcher` in `chronacle-providers`. |
| **`notify = "8"`** | `crates/chronacle-providers/Cargo.toml:24` | Already vendored & approved (unused). The watcher uses it. **No new ADR needed for `notify` itself.** |
| **`PendingWrites` loop guard** | `crates/chronacle-vault/src/outbound.rs` | `arm(key,hash)` / `matches(key,hash)` / `sweep()`, 30s TTL. Content-hash keyed, NOT consumed on match (one write → several fs events), TTL-expiring. **The watcher MUST call `matches` to ignore our own writes** — the drain already `arm`s before every write. This is the entire write→watch→write loop-break; it was built and unit-tested in D4a specifically so the watcher has something to trip. |
| **`VaultStore` / `VaultRecordStore`** ports + impls | `chronacle-core/src/vault.rs`; `LocalFsVaultStore` (providers), `SurrealVaultRecordStore` (domain) | `read`/`write`/`delete`/`list`/`metadata`; `list_all`/`load`/`get_synced_hash`/`set_synced_hash`/`clear_synced_hash`. Inbound needs to WRITE records back — the record store currently only reads records + manages the hash; you'll add record-mutation methods (or route through the existing `entity_service`/`session_service`). |
| **`vault_deleted` ×9 + `vault_sync_state`** schema | `crates/chronacle-db/src/schema/003_vault_sync.surql` | `SoftDelete` sets `vault_deleted = true`; read paths already filter `!= true`. `synced_hash` stored as string. |
| **`frontmatter::parse` / `markdown::split_body`** | `chronacle-vault/src/{frontmatter,markdown}.rs` | Inbound must PARSE a GM-edited file back into fields. `parse` already exists (used by `VaultIndex::scan`); `markdown` has the lossless body grammar (fenced compiler block vs GM-owned prose). Applying inbound means: parse the file, diff against the record, write the GM-owned parts back (never the fenced compiler block — that's compiler-owned). |

---

## 2. The two landmines — fix these BEFORE enabling `SoftDelete`

Both are harmless today (export-only defers the actions) and become **data-loss risks** the moment inbound is live. Details in `.claude/.../memory/project_vault_sync_tranche5_landmines.md`.

### L1 — Missing IPC command surface
There is **no Tauri command** to soft-delete an entity (`delete_entity` hard-deletes via `DELETE`) or to collection-scope an entity (`create_entity` hardcodes `campaign_id: String`, passes `collection_id: None`), even though `entity_service::create` supports `collection_id`. This blocked 2 of 4 real-app UI-E2E scenarios in D3b. Tranche 5 needs these commands for inbound + to make the vault the round-trippable surface it claims to be.

### L2 — `synced_hash` is per-record GLOBAL, not per vault path
`vault_sync_state.synced_hash` keys on the record thing, with no vault-path dimension. So pointing the app at a **new empty vault dir** makes every previously-synced record evaluate `decide(Some(base), db, None)` → `SoftDelete`. Export-only defers it (observed in the D3b container E2E log: a test-1 NPC "soft-deleting" in test-2's fresh dir). **Once tranche 5 acts on `SoftDelete`, switching vault folders would spuriously delete records that simply live in the old folder.**
**Fix direction:** add a vault-path (or vault-root fingerprint) column to `vault_sync_state` and scope the base to it; OR treat a `set_vault_path` change as a fresh baseline (clear/rebuild bases on switch). Decide this in tranche-5 brainstorming before touching `SoftDelete`.

---

## 3. Other carried items

- **I1 — Bulk-extraction latency (UX):** `persist_batch` (PDF extraction) passes `NoopOutbound`, so bulk-extracted entities mirror to the vault only on the next reconcile, not near-live like interactive edits. Defensible (avoids queue flooding) but asymmetric. Consider an auto-reconcile after bulk extraction, or a single coalesced bulk enqueue.
- **D-series Minors** (all non-blocking, triaged "ship as-is" — see `.superpowers/sdd/final-review-minors.md`). Optional fast-follows if convenient: exact-output test for the frontmatter/body seam; a Vitest test for `VaultSyncSettings`'s error/reject path; preserve `io::ErrorKind` in `VaultStoreError::Io`; reorder `set_vault_path` to persist the setting only after a successful reconcile.

---

## 4. Design questions to resolve in brainstorming (before planning)

1. **Conflict UX.** When `decide` returns `Conflict` (both DB and file diverged from the base), what does the GM see? Preserve-both (`.conflict.md` sidecar, like git)? A resolution UI? Last-writer-wins is wrong — it silently discards edits.
2. **What is applied inbound.** A vault `.md` has a fenced compiler-owned block (`codex_article` / rule `body`) and GM-owned prose (`summary`, `notes`). Inbound must apply ONLY the GM-owned parts back to the DB — never let a GM edit inside the fenced block overwrite the compiled article (or define what happens if they do). The lossless body grammar already separates these.
3. **Rename inbound.** The GM renames a file in Obsidian. Export already handles this (index-wins). But if the GM renames AND the DB `name` changes, that's a `name` conflict — how resolved?
4. **Watcher debounce + the guard.** `notify` fires bursts; the watcher should debounce and consult `PendingWrites::matches` to drop our own writes. Confirm the 30s TTL and content-hash keying are sufficient, or tune.
5. **Whether inbound is reconcile-driven, watcher-driven, or both.** Reconcile is the correctness guarantee; the watcher is latency. Inbound `Apply` can flow through reconcile (already computes it) triggered by the watcher — mirror the outbound design (queue is optimization, reconcile is truth).

---

## 5. Conventions & infrastructure (unchanged from tranche 4)

- **Hard constraints:** Tauri IPC only; SurrealQL only; **schema migrations are DEFINE-only / idempotent** (`run_migrations` re-runs every `.surql` on every boot — a `REMOVE` wiped `relates_to` edges once); traits for all external deps (never `std::fs`/SurrealDB/LLM directly — `chronacle-vault` must stay fs-free); Svelte 5 runes only; approved-crates-only (`notify` is already approved).
- **IDs are bare** (`"n1"`, not `"npc:n1"`) everywhere in the vault layer; `VaultRef{table,id}`, `to_thing()` = `table:id`. Keep this — the whole loop depends on it.
- **Testing layers:** Rust unit (`#[cfg(test)]`, `mockall`) + integration (`apps/desktop/src-tauri/tests/`, `mem://` SurrealDB); Vitest + `@testing-library/svelte` + `msw`; **backend E2E** = `pnpm -C apps/desktop run e2e:backend` (playwright-bdd, mocked IPC, every PR); **UI E2E** = tauri-driver Mocha, Linux-only, merge-to-main. **You CAN run the UI E2E locally on macOS via Docker:** `docker build -f apps/desktop/tests/e2e/ui/Dockerfile -t chronacle-e2e . && docker run --rm chronacle-e2e tests/e2e/ui/<spec>.mjs`. Every user-visible behavior ships `.feature` scenarios (ADR-011).
- **Full gate before ANY PR** (learned the hard way): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`, **`cargo deny check`** (don't skip — it catches advisories), `pnpm -C apps/desktop typecheck && lint && test:run && run e2e:backend`.
- **Workflow (same as D-series):** brainstorming → writing-plans → subagent-driven-development. Stacked feature branches; push new/stacked branches with an **explicit refspec** (`git push -u origin <b>:refs/heads/<b>`) or the first push overwrites the parent PR's head. One logical change per PR; reconcile local `main` by verifying `<branch>^{tree} == origin/main^{tree}` byte-identical before advancing.

---

## 6. Suggested first moves

1. **Brainstorm** the inbound design — resolve §4, especially conflict UX (Q1) and the apply-scope (Q2), and decide L2's fix (path-scoped base vs fresh-baseline-on-switch).
2. **Fix L2** as an early, isolated task (schema + base-scoping) — it gates `SoftDelete` safety.
3. **Add the L1 commands** (soft-delete, collection-scope) — small, unblocks real-app E2E coverage.
4. **Build the `NotifyWatcher`** (`VaultWatcher` impl) with debounce + `PendingWrites::matches` filtering.
5. **Turn on inbound in `reconcile.rs:114`** — replace the deferred-log arm with `Apply`/`SoftDelete` materialization (route DB writes through `entity_service`/`session_service` so validation/wikilinks/embeddings stay consistent), and `Conflict` per the chosen UX.
6. Wire the watcher into the app (spawn like `drain_loop`; respawn on `set_vault_path`).
