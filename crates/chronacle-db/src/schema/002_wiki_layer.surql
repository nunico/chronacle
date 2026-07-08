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
DEFINE FIELD OVERWRITE payload     ON TABLE lint_finding FLEXIBLE TYPE object;
DEFINE FIELD OVERWRITE created_at  ON TABLE lint_finding TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE resolved_at ON TABLE lint_finding TYPE option<datetime> DEFAULT NONE;

DEFINE INDEX OVERWRITE lint_finding_kind_idx
    ON TABLE lint_finding COLUMNS kind;
DEFINE INDEX OVERWRITE lint_finding_unresolved_idx
    ON TABLE lint_finding COLUMNS resolved_at;

-- ═══ A2a slice — Codex compiled world model (ADR-009) ═══════════════════════

-- ── Entity codex-article fields (machine-owned; user summary/notes untouched) ─
-- Repeated identically for: npc, location, faction, creature, item, event,
-- player_character, misc.
-- codex_sources entry shapes by kind:
--   { kind: "chunk", source, page_start, page_end }
--   { kind: "session", session }
--   { kind: "proposal", proposal }

DEFINE FIELD OVERWRITE codex_article     ON TABLE npc TYPE option<string> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_compiled_at ON TABLE npc TYPE option<datetime> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_stale       ON TABLE npc TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE codex_sources     ON TABLE npc FLEXIBLE TYPE array<object> DEFAULT [];

DEFINE FIELD OVERWRITE codex_article     ON TABLE location TYPE option<string> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_compiled_at ON TABLE location TYPE option<datetime> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_stale       ON TABLE location TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE codex_sources     ON TABLE location FLEXIBLE TYPE array<object> DEFAULT [];

DEFINE FIELD OVERWRITE codex_article     ON TABLE faction TYPE option<string> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_compiled_at ON TABLE faction TYPE option<datetime> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_stale       ON TABLE faction TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE codex_sources     ON TABLE faction FLEXIBLE TYPE array<object> DEFAULT [];

DEFINE FIELD OVERWRITE codex_article     ON TABLE creature TYPE option<string> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_compiled_at ON TABLE creature TYPE option<datetime> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_stale       ON TABLE creature TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE codex_sources     ON TABLE creature FLEXIBLE TYPE array<object> DEFAULT [];

DEFINE FIELD OVERWRITE codex_article     ON TABLE item TYPE option<string> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_compiled_at ON TABLE item TYPE option<datetime> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_stale       ON TABLE item TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE codex_sources     ON TABLE item FLEXIBLE TYPE array<object> DEFAULT [];

DEFINE FIELD OVERWRITE codex_article     ON TABLE event TYPE option<string> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_compiled_at ON TABLE event TYPE option<datetime> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_stale       ON TABLE event TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE codex_sources     ON TABLE event FLEXIBLE TYPE array<object> DEFAULT [];

DEFINE FIELD OVERWRITE codex_article     ON TABLE player_character TYPE option<string> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_compiled_at ON TABLE player_character TYPE option<datetime> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_stale       ON TABLE player_character TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE codex_sources     ON TABLE player_character FLEXIBLE TYPE array<object> DEFAULT [];

DEFINE FIELD OVERWRITE codex_article     ON TABLE misc TYPE option<string> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_compiled_at ON TABLE misc TYPE option<datetime> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_stale       ON TABLE misc TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE codex_sources     ON TABLE misc FLEXIBLE TYPE array<object> DEFAULT [];

-- ── Rule entries (compiled rules layer; ADR-009) ─────────────────────────────
-- body/category/page_refs/sources are compiler-owned; notes is GM-owned and
-- survives recompiles. GM corrections go through "redo with objections",
-- recorded in sources as { kind: "objection", text, at }.

DEFINE TABLE OVERWRITE rule_entry SCHEMAFULL;
DEFINE FIELD OVERWRITE collection  ON TABLE rule_entry TYPE record<collection>;
DEFINE FIELD OVERWRITE name        ON TABLE rule_entry TYPE string;
DEFINE FIELD OVERWRITE category    ON TABLE rule_entry TYPE string
    ASSERT $value IN ['mechanic', 'ability', 'state', 'procedure', 'resource', 'statistic', 'entry'];
DEFINE FIELD OVERWRITE body        ON TABLE rule_entry TYPE string;
DEFINE FIELD OVERWRITE notes       ON TABLE rule_entry TYPE string | NULL DEFAULT NULL;
DEFINE FIELD OVERWRITE page_refs   ON TABLE rule_entry FLEXIBLE TYPE array<object> DEFAULT [];
DEFINE FIELD OVERWRITE sources     ON TABLE rule_entry FLEXIBLE TYPE array<object> DEFAULT [];
DEFINE FIELD OVERWRITE compiled_at ON TABLE rule_entry TYPE datetime;
DEFINE FIELD OVERWRITE stale       ON TABLE rule_entry TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE embedding   ON TABLE rule_entry TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD OVERWRITE embed_model ON TABLE rule_entry TYPE string | NULL DEFAULT NULL;
DEFINE FIELD OVERWRITE created_at  ON TABLE rule_entry TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at  ON TABLE rule_entry TYPE datetime DEFAULT time::now();
DEFINE INDEX OVERWRITE rule_entry_collection_idx ON TABLE rule_entry COLUMNS collection;
DEFINE INDEX OVERWRITE rule_entry_name_idx       ON TABLE rule_entry COLUMNS collection, name UNIQUE;
DEFINE INDEX OVERWRITE rule_entry_embedding_idx  ON TABLE rule_entry FIELDS embedding MTREE DIMENSION 768 DIST COSINE;

-- ── Codex proposals (write-back review queue; ADR-009) ───────────────────────
-- payload: { proposed_text, rationale } for *_update kinds; full draft object
-- for new_* kinds. origin: { kind: "chat", message } | { kind: "session",
-- session } | { kind: "manual" }. target unset for new_* kinds.

DEFINE TABLE OVERWRITE codex_proposal SCHEMAFULL;
DEFINE FIELD OVERWRITE kind        ON TABLE codex_proposal TYPE string
    ASSERT $value IN ['entity_article_update', 'entity_notes_update',
                      'rule_entry_update', 'new_entity', 'new_rule_entry'];
DEFINE FIELD OVERWRITE target      ON TABLE codex_proposal TYPE option<record> DEFAULT NONE;
DEFINE FIELD OVERWRITE collection  ON TABLE codex_proposal TYPE record<collection>;
DEFINE FIELD OVERWRITE campaign    ON TABLE codex_proposal TYPE option<record<campaign>> DEFAULT NONE;
DEFINE FIELD OVERWRITE payload     ON TABLE codex_proposal FLEXIBLE TYPE object;
DEFINE FIELD OVERWRITE origin      ON TABLE codex_proposal FLEXIBLE TYPE object;
DEFINE FIELD OVERWRITE status      ON TABLE codex_proposal TYPE string DEFAULT 'pending'
    ASSERT $value IN ['pending', 'accepted', 'rejected'];
DEFINE FIELD OVERWRITE created_at  ON TABLE codex_proposal TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE resolved_at ON TABLE codex_proposal TYPE option<datetime> DEFAULT NONE;
DEFINE INDEX OVERWRITE codex_proposal_status_idx ON TABLE codex_proposal COLUMNS status;

-- ── lint_finding: kinds added from A2b onward ────────────────────────────────
-- (no schema change — kind vocabulary documented here)
--   * "scope_violation"  — payload = { edge: option, from, to,
--                                       from_collection, to_collection }
--                                       produced by the C2a manual lint pass
--                                       (codex_service::lint::run_lint_campaign)
--   * "broken_wikilink"  — payload = { entity, link_text }
--                                       produced by the C2a manual lint pass
--   * "stale_article"    — payload = { entity, reason }
--                                       produced by the C2a manual lint pass
--   * "duplicate_entity" — payload = { a, b, similarity }
--                                       produced by the C2a manual lint pass
