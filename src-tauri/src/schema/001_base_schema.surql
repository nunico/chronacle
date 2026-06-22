-- 001_base_schema.surql
-- Single consolidated schema for Chronacle (squashed from Phase 1-3 migrations).
-- All DEFINE statements are safe to re-run on every app startup: SCHEMAFULL tables
-- use plain DEFINE (which is idempotent), and the relates_to edge table uses
-- DEFINE TABLE/FIELD OVERWRITE so re-assertion never drops existing edges.

-- ── Campaigns ────────────────────────────────────────────────────────────────

DEFINE TABLE campaign SCHEMAFULL;
DEFINE FIELD name       ON campaign TYPE string;
DEFINE FIELD system     ON campaign TYPE string;
DEFINE FIELD created_at ON campaign TYPE datetime;
DEFINE FIELD updated_at ON campaign TYPE datetime;

-- ── Collections ──────────────────────────────────────────────────────────────

DEFINE TABLE collection SCHEMAFULL;
DEFINE FIELD name        ON collection TYPE string;
DEFINE FIELD description ON collection TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at  ON collection TYPE datetime;
DEFINE FIELD updated_at  ON collection TYPE datetime;

-- campaign subscribes_to collection (many-to-many via edge).
DEFINE TABLE subscribes_to SCHEMAFULL TYPE RELATION FROM campaign TO collection;
DEFINE FIELD created_at ON subscribes_to TYPE datetime;
DEFINE INDEX unique_subscription ON TABLE subscribes_to COLUMNS in, out UNIQUE;

-- ── Sources & Chunks ─────────────────────────────────────────────────────────

DEFINE TABLE source SCHEMAFULL;
-- source.campaign is nullable: collection-scoped sources omit it.
DEFINE FIELD campaign      ON source TYPE record<campaign> | NULL DEFAULT NULL;
-- source.collection is required: all sources belong to exactly one collection.
DEFINE FIELD collection    ON source TYPE record<collection>;
DEFINE FIELD filename      ON source TYPE string;
DEFINE FIELD display_name  ON source TYPE string;
DEFINE FIELD source_type   ON source TYPE string ASSERT $value IN ['rules', 'lore', 'supplement'];
DEFINE FIELD page_count    ON source TYPE int;
DEFINE FIELD indexed_at    ON source TYPE datetime;
DEFINE FIELD index_status  ON source TYPE string ASSERT $value IN ['pending', 'indexing', 'done', 'error'];
DEFINE FIELD embed_model   ON source TYPE string;

DEFINE TABLE chunk SCHEMAFULL;
DEFINE FIELD source          ON chunk TYPE record<source>;
-- chunk.campaign is optional<record> to support legacy code paths; new code uses collection.
DEFINE FIELD campaign        ON chunk TYPE option<record<campaign>> DEFAULT NONE;
DEFINE FIELD collection      ON chunk TYPE record<collection>;
DEFINE FIELD text            ON chunk TYPE string;
DEFINE FIELD page_start      ON chunk TYPE int;
DEFINE FIELD page_end        ON chunk TYPE int;
DEFINE FIELD section_heading ON chunk TYPE string;
DEFINE FIELD source_type     ON chunk TYPE string;
DEFINE FIELD embedding       ON chunk TYPE array<float>;
DEFINE FIELD embed_model     ON chunk TYPE string;

DEFINE INDEX chunk_embedding_idx ON chunk FIELDS embedding MTREE DIMENSION 768 DIST COSINE;

-- ── Sessions ─────────────────────────────────────────────────────────────────

DEFINE TABLE session SCHEMAFULL;
DEFINE FIELD campaign       ON session TYPE record<campaign> | NULL DEFAULT NULL;
DEFINE FIELD session_number ON session TYPE int;
DEFINE FIELD title          ON session TYPE string;
DEFINE FIELD date_played    ON session TYPE string;
DEFINE FIELD notes          ON session TYPE string;
DEFINE FIELD created_at     ON session TYPE datetime;
DEFINE FIELD updated_at     ON session TYPE datetime DEFAULT time::now();
DEFINE FIELD embedding      ON TABLE session TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD embed_model    ON TABLE session TYPE string | NULL DEFAULT NULL;

DEFINE INDEX idx_session_campaign ON session FIELDS campaign;
DEFINE INDEX session_embedding_idx ON TABLE session FIELDS embedding MTREE DIMENSION 768 DIST COSINE;

-- ── Messages ─────────────────────────────────────────────────────────────────

DEFINE TABLE message SCHEMAFULL;
-- DEFAULT NULL lets inserts omit campaign (e.g. global / no-campaign context).
DEFINE FIELD campaign   ON message TYPE record<campaign> | NULL DEFAULT NULL;
DEFINE FIELD role       ON message TYPE string ASSERT $value IN ['user', 'assistant', 'system'];
DEFINE FIELD content    ON message TYPE string;
DEFINE FIELD citations  ON message TYPE array<object>;
DEFINE FIELD created_at ON message TYPE datetime;

-- ── Settings ─────────────────────────────────────────────────────────────────

DEFINE TABLE setting SCHEMAFULL;
DEFINE FIELD value ON setting TYPE string;

-- ── Custom Providers ─────────────────────────────────────────────────────────

DEFINE TABLE custom_provider SCHEMAFULL;
DEFINE FIELD name          ON custom_provider TYPE string;
DEFINE FIELD provider_type ON custom_provider TYPE string ASSERT $value IN ['openai', 'anthropic'];
DEFINE FIELD base_url      ON custom_provider TYPE string;
DEFINE FIELD api_key       ON custom_provider TYPE string;
DEFINE FIELD created_at    ON custom_provider TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at    ON custom_provider TYPE datetime;

DEFINE INDEX unique_provider_name ON custom_provider FIELDS name UNIQUE;

DEFINE TABLE custom_provider_model SCHEMAFULL;
DEFINE FIELD provider     ON custom_provider_model TYPE record<custom_provider>;
DEFINE FIELD model_id     ON custom_provider_model TYPE string;
DEFINE FIELD display_name ON custom_provider_model TYPE string;
DEFINE FIELD created_at   ON custom_provider_model TYPE datetime DEFAULT time::now();

DEFINE INDEX unique_model_per_provider ON custom_provider_model FIELDS provider, model_id UNIQUE;

-- ── Graph: Entity Node Tables ─────────────────────────────────────────────────
-- Each entity type is its own table (graph node). Scope (campaign vs. collection)
-- is expressed via in_campaign / in_collection edge tables (see below).
-- Shared fields: name, summary, notes, created_at, updated_at, embedding, embed_model.

DEFINE TABLE npc SCHEMAFULL;
DEFINE FIELD name        ON npc TYPE string;
DEFINE FIELD summary     ON npc TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes       ON npc TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at  ON npc TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at  ON npc TYPE datetime DEFAULT time::now();
DEFINE FIELD embedding   ON TABLE npc TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD embed_model ON TABLE npc TYPE string | NULL DEFAULT NULL;
DEFINE INDEX npc_embedding_idx ON TABLE npc FIELDS embedding MTREE DIMENSION 768 DIST COSINE;

DEFINE TABLE location SCHEMAFULL;
DEFINE FIELD name        ON location TYPE string;
DEFINE FIELD summary     ON location TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes       ON location TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at  ON location TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at  ON location TYPE datetime DEFAULT time::now();
DEFINE FIELD embedding   ON TABLE location TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD embed_model ON TABLE location TYPE string | NULL DEFAULT NULL;
DEFINE INDEX location_embedding_idx ON TABLE location FIELDS embedding MTREE DIMENSION 768 DIST COSINE;

DEFINE TABLE faction SCHEMAFULL;
DEFINE FIELD name        ON faction TYPE string;
DEFINE FIELD summary     ON faction TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes       ON faction TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at  ON faction TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at  ON faction TYPE datetime DEFAULT time::now();
DEFINE FIELD embedding   ON TABLE faction TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD embed_model ON TABLE faction TYPE string | NULL DEFAULT NULL;
DEFINE INDEX faction_embedding_idx ON TABLE faction FIELDS embedding MTREE DIMENSION 768 DIST COSINE;

DEFINE TABLE creature SCHEMAFULL;
DEFINE FIELD name        ON creature TYPE string;
DEFINE FIELD summary     ON creature TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes       ON creature TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at  ON creature TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at  ON creature TYPE datetime DEFAULT time::now();
DEFINE FIELD embedding   ON TABLE creature TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD embed_model ON TABLE creature TYPE string | NULL DEFAULT NULL;
DEFINE INDEX creature_embedding_idx ON TABLE creature FIELDS embedding MTREE DIMENSION 768 DIST COSINE;

DEFINE TABLE item SCHEMAFULL;
DEFINE FIELD name        ON item TYPE string;
DEFINE FIELD summary     ON item TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes       ON item TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at  ON item TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at  ON item TYPE datetime DEFAULT time::now();
DEFINE FIELD embedding   ON TABLE item TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD embed_model ON TABLE item TYPE string | NULL DEFAULT NULL;
DEFINE INDEX item_embedding_idx ON TABLE item FIELDS embedding MTREE DIMENSION 768 DIST COSINE;

DEFINE TABLE event SCHEMAFULL;
DEFINE FIELD name           ON event TYPE string;
DEFINE FIELD summary        ON event TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes          ON event TYPE string | NULL DEFAULT NULL;
DEFINE FIELD date_start     ON event TYPE string | NULL DEFAULT NULL;
DEFINE FIELD date_end       ON event TYPE string | NULL DEFAULT NULL;
DEFINE FIELD is_ongoing     ON event TYPE bool DEFAULT false;
DEFINE FIELD sequence_index ON event TYPE int | NULL DEFAULT NULL;
DEFINE FIELD era            ON event TYPE string | NULL DEFAULT NULL;
DEFINE FIELD duration_label ON event TYPE string | NULL DEFAULT NULL;
DEFINE FIELD session        ON event TYPE record<session> | NULL DEFAULT NULL;
DEFINE FIELD created_at     ON event TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at     ON event TYPE datetime DEFAULT time::now();
DEFINE FIELD embedding      ON TABLE event TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD embed_model    ON TABLE event TYPE string | NULL DEFAULT NULL;
DEFINE INDEX event_embedding_idx ON TABLE event FIELDS embedding MTREE DIMENSION 768 DIST COSINE;

DEFINE TABLE player_character SCHEMAFULL;
DEFINE FIELD name             ON player_character TYPE string;
DEFINE FIELD summary          ON player_character TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes            ON player_character TYPE string | NULL DEFAULT NULL;
DEFINE FIELD player_name      ON player_character TYPE string | NULL DEFAULT NULL;
DEFINE FIELD character_class  ON player_character TYPE string | NULL DEFAULT NULL;
DEFINE FIELD character_level  ON player_character TYPE int | NULL DEFAULT NULL;
DEFINE FIELD status           ON player_character TYPE string | NULL DEFAULT NULL
  ASSERT $value = NONE OR $value IN ['active', 'retired', 'deceased', 'missing', 'on_hiatus'];
DEFINE FIELD created_at       ON player_character TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at       ON player_character TYPE datetime DEFAULT time::now();
DEFINE FIELD embedding        ON TABLE player_character TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD embed_model      ON TABLE player_character TYPE string | NULL DEFAULT NULL;
DEFINE INDEX player_character_embedding_idx ON TABLE player_character FIELDS embedding MTREE DIMENSION 768 DIST COSINE;

DEFINE TABLE misc SCHEMAFULL;
DEFINE FIELD name        ON misc TYPE string;
DEFINE FIELD summary     ON misc TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes       ON misc TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at  ON misc TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at  ON misc TYPE datetime DEFAULT time::now();
DEFINE FIELD embedding   ON TABLE misc TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD embed_model ON TABLE misc TYPE string | NULL DEFAULT NULL;
DEFINE INDEX misc_embedding_idx ON TABLE misc FIELDS embedding MTREE DIMENSION 768 DIST COSINE;

-- ── Graph: Scope Edge Tables ─────────────────────────────────────────────────
-- Entities belong to either a campaign or a collection (not both; enforced via
-- a UNIQUE index on `out`). Service layer inserts the correct edge at create time.

DEFINE TABLE in_campaign SCHEMAFULL TYPE RELATION
    FROM campaign
    TO   npc | location | faction | creature | item | event | player_character | misc;
DEFINE FIELD created_at ON in_campaign TYPE datetime DEFAULT time::now();
-- Each entity can belong to at most one campaign.
DEFINE INDEX unique_entity_campaign ON TABLE in_campaign COLUMNS out UNIQUE;

DEFINE TABLE in_collection SCHEMAFULL TYPE RELATION
    FROM collection
    TO   npc | location | faction | creature | item | event | player_character | misc;
DEFINE FIELD created_at ON in_collection TYPE datetime DEFAULT time::now();
-- Each entity can belong to at most one collection.
DEFINE INDEX unique_entity_collection ON TABLE in_collection COLUMNS out UNIQUE;

-- ── Graph: Relationship Edge Table ───────────────────────────────────────────
-- Generic directed edge between any two entity nodes.
-- FROM/TO lists all 8 node tables explicitly: SurrealDB v2's record<ANY> type
-- check does not work correctly for heterogeneous RELATE in v2.6.x.
-- OVERWRITE on TABLE and FIELDs: re-asserts the definition on every startup
-- without dropping existing edges. Safe to run against a live database.

DEFINE TABLE OVERWRITE relates_to SCHEMAFULL TYPE RELATION
    FROM npc | location | faction | creature | item | event | player_character | misc
    TO   npc | location | faction | creature | item | event | player_character | misc;
DEFINE FIELD OVERWRITE rel_type   ON relates_to TYPE string;
DEFINE FIELD OVERWRITE notes      ON relates_to TYPE string | NULL DEFAULT NULL;
DEFINE FIELD OVERWRITE created_at ON relates_to TYPE datetime DEFAULT time::now();
