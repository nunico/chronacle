-- 006_collection_entities.surql
-- Migrate campaign scalar field → in_campaign edge; add in_collection edge;
-- add embedding fields and MTREE indexes for all 8 entity tables.

-- ── Edge tables ──────────────────────────────────────────────────────────────

DEFINE TABLE in_campaign SCHEMAFULL TYPE RELATION
    FROM campaign
    TO   npc | location | faction | creature | item | event | player_character | misc;
DEFINE FIELD created_at ON in_campaign TYPE datetime DEFAULT time::now();
DEFINE INDEX unique_entity_campaign ON TABLE in_campaign COLUMNS out UNIQUE;

DEFINE TABLE in_collection SCHEMAFULL TYPE RELATION
    FROM collection
    TO   npc | location | faction | creature | item | event | player_character | misc;
DEFINE FIELD created_at ON in_collection TYPE datetime DEFAULT time::now();
DEFINE INDEX unique_entity_collection ON TABLE in_collection COLUMNS out UNIQUE;

-- ── Data migration: scalar campaign field → in_campaign edges ────────────────
-- For each entity table, bulk-insert in_campaign edges from existing records
-- that still carry the scalar campaign field. INSERT IGNORE skips duplicates
-- (idempotent on re-run).

INSERT IGNORE INTO in_campaign (SELECT campaign AS in, id AS out, time::now() AS created_at FROM npc WHERE campaign IS NOT NONE);
INSERT IGNORE INTO in_campaign (SELECT campaign AS in, id AS out, time::now() AS created_at FROM location WHERE campaign IS NOT NONE);
INSERT IGNORE INTO in_campaign (SELECT campaign AS in, id AS out, time::now() AS created_at FROM faction WHERE campaign IS NOT NONE);
INSERT IGNORE INTO in_campaign (SELECT campaign AS in, id AS out, time::now() AS created_at FROM creature WHERE campaign IS NOT NONE);
INSERT IGNORE INTO in_campaign (SELECT campaign AS in, id AS out, time::now() AS created_at FROM item WHERE campaign IS NOT NONE);
INSERT IGNORE INTO in_campaign (SELECT campaign AS in, id AS out, time::now() AS created_at FROM event WHERE campaign IS NOT NONE);
INSERT IGNORE INTO in_campaign (SELECT campaign AS in, id AS out, time::now() AS created_at FROM player_character WHERE campaign IS NOT NONE);
INSERT IGNORE INTO in_campaign (SELECT campaign AS in, id AS out, time::now() AS created_at FROM misc WHERE campaign IS NOT NONE);

-- ── Remove scalar campaign field from all entity tables ──────────────────────

REMOVE FIELD IF EXISTS campaign ON TABLE npc;
REMOVE FIELD IF EXISTS campaign ON TABLE location;
REMOVE FIELD IF EXISTS campaign ON TABLE faction;
REMOVE FIELD IF EXISTS campaign ON TABLE creature;
REMOVE FIELD IF EXISTS campaign ON TABLE item;
REMOVE FIELD IF EXISTS campaign ON TABLE event;
REMOVE FIELD IF EXISTS campaign ON TABLE player_character;
REMOVE FIELD IF EXISTS campaign ON TABLE misc;

-- ── Embedding fields (ADR-003 pattern — matches source/chunk) ─────────────────

DEFINE FIELD embedding   ON TABLE npc              TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD embed_model ON TABLE npc              TYPE string | NULL DEFAULT NULL;
DEFINE FIELD embedding   ON TABLE location         TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD embed_model ON TABLE location         TYPE string | NULL DEFAULT NULL;
DEFINE FIELD embedding   ON TABLE faction          TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD embed_model ON TABLE faction          TYPE string | NULL DEFAULT NULL;
DEFINE FIELD embedding   ON TABLE creature         TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD embed_model ON TABLE creature         TYPE string | NULL DEFAULT NULL;
DEFINE FIELD embedding   ON TABLE item             TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD embed_model ON TABLE item             TYPE string | NULL DEFAULT NULL;
DEFINE FIELD embedding   ON TABLE event            TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD embed_model ON TABLE event            TYPE string | NULL DEFAULT NULL;
DEFINE FIELD embedding   ON TABLE player_character TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD embed_model ON TABLE player_character TYPE string | NULL DEFAULT NULL;
DEFINE FIELD embedding   ON TABLE misc             TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD embed_model ON TABLE misc             TYPE string | NULL DEFAULT NULL;

-- ── MTREE vector indexes (cosine, 768-dim, matches nomic-embed-text-v1.5) ─────

DEFINE INDEX npc_embedding_idx              ON TABLE npc              FIELDS embedding MTREE DIMENSION 768 DIST COSINE;
DEFINE INDEX location_embedding_idx         ON TABLE location         FIELDS embedding MTREE DIMENSION 768 DIST COSINE;
DEFINE INDEX faction_embedding_idx          ON TABLE faction          FIELDS embedding MTREE DIMENSION 768 DIST COSINE;
DEFINE INDEX creature_embedding_idx         ON TABLE creature         FIELDS embedding MTREE DIMENSION 768 DIST COSINE;
DEFINE INDEX item_embedding_idx             ON TABLE item             FIELDS embedding MTREE DIMENSION 768 DIST COSINE;
DEFINE INDEX event_embedding_idx            ON TABLE event            FIELDS embedding MTREE DIMENSION 768 DIST COSINE;
DEFINE INDEX player_character_embedding_idx ON TABLE player_character FIELDS embedding MTREE DIMENSION 768 DIST COSINE;
DEFINE INDEX misc_embedding_idx             ON TABLE misc             FIELDS embedding MTREE DIMENSION 768 DIST COSINE;
