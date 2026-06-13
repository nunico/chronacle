-- 008_is_gm_only.surql
-- Phase 2: introduce the `is_gm_only` flag (GM-secret material).
--
-- Single-user app: retrieval NEVER filters GM-only content out — the flag only
-- tags records so the UI can show a shield indicator and a future player-safe
-- export can strip them. Chunks inherit the flag from their source at index
-- time (set in the vector-store upsert), so GM-secret PDFs propagate into the
-- vector index. Defaults to false so all existing rows stay GM-visible.

DEFINE FIELD is_gm_only ON TABLE source  TYPE bool DEFAULT false;
DEFINE FIELD is_gm_only ON TABLE chunk   TYPE bool DEFAULT false;
DEFINE FIELD is_gm_only ON TABLE session TYPE bool DEFAULT false;

DEFINE FIELD is_gm_only ON TABLE npc              TYPE bool DEFAULT false;
DEFINE FIELD is_gm_only ON TABLE location         TYPE bool DEFAULT false;
DEFINE FIELD is_gm_only ON TABLE faction          TYPE bool DEFAULT false;
DEFINE FIELD is_gm_only ON TABLE creature         TYPE bool DEFAULT false;
DEFINE FIELD is_gm_only ON TABLE item             TYPE bool DEFAULT false;
DEFINE FIELD is_gm_only ON TABLE event            TYPE bool DEFAULT false;
DEFINE FIELD is_gm_only ON TABLE player_character TYPE bool DEFAULT false;
DEFINE FIELD is_gm_only ON TABLE misc             TYPE bool DEFAULT false;
