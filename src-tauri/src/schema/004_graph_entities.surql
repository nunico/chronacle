-- 004_graph_entities.surql
-- Replaces the Phase 1 stub entity table with proper graph node tables.
-- Each entity type is its own SurrealDB table (graph node), allowing
-- type-safe fields and natural graph traversal via relates_to edges.

-- Drop stub table and old relation from Phase 1
REMOVE TABLE IF EXISTS entity;
REMOVE TABLE IF EXISTS relates_to;

-- Generic graph edge (connects any two node types.
-- FROM/TO lists all entity tables explicitly because SurrealDB v2's record<ANY>
-- type check does not work correctly for heterogeneous RELATE in v2.6.x.
DEFINE TABLE relates_to SCHEMAFULL TYPE RELATION
    FROM npc | location | faction | creature | item | event | player_character | misc
    TO   npc | location | faction | creature | item | event | player_character | misc;
DEFINE FIELD rel_type   ON relates_to TYPE string;
DEFINE FIELD notes      ON relates_to TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at ON relates_to TYPE datetime DEFAULT time::now();

-- Shared fields (manually repeated for each node table below):
-- Each table gets: campaign, name, summary, notes, created_at, updated_at

DEFINE TABLE npc SCHEMAFULL;
DEFINE FIELD campaign   ON npc TYPE record<campaign> | NULL DEFAULT NULL;
DEFINE FIELD name       ON npc TYPE string;
DEFINE FIELD summary    ON npc TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes      ON npc TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at ON npc TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON npc TYPE datetime DEFAULT time::now();

DEFINE TABLE location SCHEMAFULL;
DEFINE FIELD campaign   ON location TYPE record<campaign> | NULL DEFAULT NULL;
DEFINE FIELD name       ON location TYPE string;
DEFINE FIELD summary    ON location TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes      ON location TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at ON location TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON location TYPE datetime DEFAULT time::now();

DEFINE TABLE faction SCHEMAFULL;
DEFINE FIELD campaign   ON faction TYPE record<campaign> | NULL DEFAULT NULL;
DEFINE FIELD name       ON faction TYPE string;
DEFINE FIELD summary    ON faction TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes      ON faction TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at ON faction TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON faction TYPE datetime DEFAULT time::now();

DEFINE TABLE creature SCHEMAFULL;
DEFINE FIELD campaign   ON creature TYPE record<campaign> | NULL DEFAULT NULL;
DEFINE FIELD name       ON creature TYPE string;
DEFINE FIELD summary    ON creature TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes      ON creature TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at ON creature TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON creature TYPE datetime DEFAULT time::now();

DEFINE TABLE item SCHEMAFULL;
DEFINE FIELD campaign   ON item TYPE record<campaign> | NULL DEFAULT NULL;
DEFINE FIELD name       ON item TYPE string;
DEFINE FIELD summary    ON item TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes      ON item TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at ON item TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON item TYPE datetime DEFAULT time::now();

DEFINE TABLE event SCHEMAFULL;
DEFINE FIELD campaign        ON event TYPE record<campaign> | NULL DEFAULT NULL;
DEFINE FIELD name            ON event TYPE string;
DEFINE FIELD summary         ON event TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes           ON event TYPE string | NULL DEFAULT NULL;
DEFINE FIELD date_start      ON event TYPE string | NULL DEFAULT NULL;
DEFINE FIELD date_end        ON event TYPE string | NULL DEFAULT NULL;
DEFINE FIELD is_ongoing      ON event TYPE bool DEFAULT false;
DEFINE FIELD sequence_index  ON event TYPE int | NULL DEFAULT NULL;
DEFINE FIELD era             ON event TYPE string | NULL DEFAULT NULL;
DEFINE FIELD duration_label  ON event TYPE string | NULL DEFAULT NULL;
DEFINE FIELD session         ON event TYPE record<session> | NULL DEFAULT NULL;
DEFINE FIELD created_at      ON event TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at      ON event TYPE datetime DEFAULT time::now();

DEFINE TABLE player_character SCHEMAFULL;
DEFINE FIELD campaign         ON player_character TYPE record<campaign> | NULL DEFAULT NULL;
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

DEFINE TABLE misc SCHEMAFULL;
DEFINE FIELD campaign   ON misc TYPE record<campaign> | NULL DEFAULT NULL;
DEFINE FIELD name       ON misc TYPE string;
DEFINE FIELD summary    ON misc TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes      ON misc TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at ON misc TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON misc TYPE datetime DEFAULT time::now();
