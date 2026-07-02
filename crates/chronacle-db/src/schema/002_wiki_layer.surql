-- 002_wiki_layer.surql
-- LLM Wiki compiled knowledge layer — PR-A1a slice.
--
-- A1a introduces:
--   * `collection.owner_campaign` — optional back-reference from a collection
--     to the campaign that exclusively owns it. Unset means "regular
--     (shareable) collection", matching all pre-A1 collections.
--   * `lint_finding` — table stub used from A1a onward to record data-layer
--     findings the user should review. In A1a only one `kind` is produced
--     (`orphaned_edge`, on campaign-delete → convert-to-regular). Later PRs
--     will add more `kind`s.
--
-- All statements use `OVERWRITE` so re-assertion on every startup is a
-- no-op that never drops existing rows or indexes.

-- ── Collections: owner_campaign ──────────────────────────────────────────────

DEFINE FIELD OVERWRITE owner_campaign ON TABLE collection
    TYPE option<record<campaign>> DEFAULT NONE;

DEFINE INDEX OVERWRITE collection_owner_campaign_idx
    ON TABLE collection COLUMNS owner_campaign;

-- ── Lint findings ────────────────────────────────────────────────────────────
-- Deliberately loose schema: `payload` is an opaque object whose shape depends
-- on `kind`. Downstream code is responsible for matching payloads to kinds.
-- Kinds populated in A1a:
--   * "orphaned_edge" — payload = { campaign_id, collection_id, edge_id,
--                                    from, to, rel_type }

DEFINE TABLE OVERWRITE lint_finding SCHEMAFULL;
DEFINE FIELD OVERWRITE kind        ON TABLE lint_finding TYPE string;
DEFINE FIELD OVERWRITE payload     ON TABLE lint_finding TYPE object;
DEFINE FIELD OVERWRITE created_at  ON TABLE lint_finding TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE resolved_at ON TABLE lint_finding TYPE option<datetime> DEFAULT NONE;

DEFINE INDEX OVERWRITE lint_finding_kind_idx
    ON TABLE lint_finding COLUMNS kind;
DEFINE INDEX OVERWRITE lint_finding_unresolved_idx
    ON TABLE lint_finding COLUMNS resolved_at;
