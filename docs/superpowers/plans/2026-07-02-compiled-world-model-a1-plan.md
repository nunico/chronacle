# Compiled World Model — PR-A1 Implementation Plan

**Date:** 2026-07-02
**Status:** In progress
**Spec:** `docs/superpowers/specs/2026-07-02-compiled-world-model-a1-design.md`

Ordered, TDD-first. Each step is a single logical commit unless noted.

## Order of work

1. **Migration file.** Add `src-tauri/src/schema/002_wiki_layer.surql` with
   only the A1 slice: `collection.owner_campaign` (+ index) and
   `lint_finding` table (+ fields + indexes). Register it in `schema/mod.rs`
   next to `001_base_schema.surql`. All `DEFINE ... OVERWRITE`.

2. **Schema tests.** New file
   `src-tauri/tests/schema_wiki_layer_a1_test.rs`:
   - `migration_is_idempotent_when_run_twice`
   - `collection_owner_campaign_is_optional_and_defaults_none`
   - `lint_finding_accepts_orphaned_edge_row`

3. **`OnOwnedCollection` enum + service signature change.**
   In `src-tauri/src/services/campaign_service.rs`:
   ```rust
   #[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum OnOwnedCollection { Delete, ConvertToRegular }
   ```
   Rename existing `delete(db, id)` to accept `on_owned_collection`.
   The existing `campaign_service_test.rs::delete_campaign_removes_it_from_listing`
   test is updated to pass `OnOwnedCollection::Delete`.

4. **`collection_service::owned_by`.** New helper:
   ```rust
   pub async fn owned_by(db: &Surreal<Db>, campaign_id: &str)
       -> Result<Option<Collection>, String>;
   ```
   Uses the new `collection_owner_campaign_idx`. Tested in
   `collection_service::tests`.

5. **Auto-create owned collection on campaign create.**
   `campaign_service::create` now, in one `.query()` block:
   ```
   BEGIN;
     LET $c   = CREATE campaign SET name = $name, system = $system;
     LET $col = CREATE collection SET name = $name, owner_campaign = $c[0].id;
     RELATE $c[0].id->subscribes_to->$col[0].id;
     RETURN $c;
   COMMIT;
   ```
   New tests in `campaign_service_test.rs`:
   - `creating_campaign_auto_creates_owned_collection_with_matching_name`
   - `creating_campaign_auto_subscribes_to_owned_collection`
   - `owned_collection_lookup_returns_it`

6. **Cascade path.** The existing `collection_service::delete` is *guarded*
   (refuses if non-empty); it is not a content-teardown helper. A1 adds a
   new private helper
   `collection_service::hard_delete_with_content(db, collection_id)` that
   removes, in one query batch:
   - `chunk` rows via `WHERE source IN (SELECT id FROM source WHERE collection = $c)`
   - `source` rows via `WHERE collection = $c`
   - `in_collection` edges via `WHERE in = $c` (and remember the entity ids)
   - `relates_to` edges touching those entities (either endpoint)
   - the entity rows themselves
   - the `collection` row itself
   Source blobs on disk are **not** touched. This mirrors the deliberate
   scoping approved in the design.
   New tests in `campaign_service_test.rs`:
   - `delete_cascade_removes_owned_collection`
   - `delete_cascade_removes_entities_inside_owned_collection`
   - `delete_cascade_leaves_regular_collections_untouched`

7. **Convert path.** Implement the orphan-edge query. `in_collection` is a
   `RELATION FROM collection TO entity`, so the entity-set of the owned
   collection is:
   ```
   LET $entities = (SELECT VALUE out FROM in_collection
                    WHERE in = type::thing('collection', $cid));
   ```
   The orphaned edges are those whose both endpoints are inside:
   ```
   LET $edges = SELECT id, in, out, rel_type FROM relates_to
                WHERE in IN $entities AND out IN $entities;
   FOR $e IN $edges {
     CREATE lint_finding SET
       kind = 'orphaned_edge',
       payload = { campaign_id: $cam, collection_id: $cid,
                   edge_id: $e.id, from: $e.in, to: $e.out,
                   rel_type: $e.rel_type };
     DELETE $e.id;
   };
   UPDATE type::thing('collection', $cid) SET owner_campaign = NONE;
   ```
   New tests:
   - `delete_convert_keeps_owned_collection_but_drops_owner_field`
   - `delete_convert_orphans_only_intra_owned_edges`
   - `delete_convert_logs_orphaned_edge_findings`
   - `delete_convert_preserves_edges_crossing_into_regular_collections`

8. **Command layer.** Update `commands/campaign_commands.rs::delete_campaign`
   to take `on_owned_collection: OnOwnedCollection` and forward. New thin
   test that the command errors when the parameter is absent (via serde).

9. **Frontend.** Update `src/lib/commands.ts::deleteCampaign(id, mode)` and
   `src/views/CampaignView.svelte::removeCampaign` to present the two-mode
   choice. No new component — a second `confirm()` after the first, plus
   inline copy. UI polish (dedicated modal) is out of scope for A1; the
   choice must exist, not be pretty.

10. **Verification.**
    - `cargo fmt --all` (must be clean).
    - `cargo clippy --workspace --all-targets -- -D warnings`.
    - `cargo test --workspace`.
    - Manually run through create → delete-cascade and
      create → delete-convert flows once in `tauri dev`.

## Commits (planned, ≤72-char subjects)

1. `feat(schema): add collection.owner_campaign + lint_finding stub`
2. `test(schema): cover 002 migration idempotency and new fields`
3. `feat(campaign): auto-create owned collection on campaign create`
4. `feat(campaign): two-mode delete with orphan-edge logging`
5. `feat(commands): require on_owned_collection choice on delete_campaign`
6. `feat(ui): ask cascade-vs-convert when deleting a campaign`
7. `docs(a1): design + plan for campaign-owned collections`

## Rollback

The migration is purely additive and uses `DEFINE ... OVERWRITE`. Reverting
the code without reverting the migration leaves an unused `owner_campaign`
field and an empty `lint_finding` table; both are harmless. The migration
file itself can be reverted in a follow-up commit if needed.
