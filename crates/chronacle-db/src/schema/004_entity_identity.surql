-- ── Tranche 6: entity identity ───────────────────────────────────────────────
-- `aliases` — GM-owned alternate names. A name variant is not a new entity; it
-- is another name for an existing one. Populated by merge, by confirming a
-- suggestion, by fuzzy auto-resolve, or by the GM directly (in-app or in the
-- vault frontmatter). Honored by wikilink resolution and duplicate detection.
--
-- DEFINE-only and idempotent: run_migrations re-runs every .surql on EVERY
-- boot. Never add a REMOVE here (one once wiped every relationship edge).

DEFINE FIELD OVERWRITE aliases ON TABLE npc              TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE aliases ON TABLE location         TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE aliases ON TABLE faction          TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE aliases ON TABLE creature         TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE aliases ON TABLE item             TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE aliases ON TABLE event            TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE aliases ON TABLE player_character TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE aliases ON TABLE misc             TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE aliases ON TABLE rule_entry       TYPE array<string> DEFAULT [];

-- New lint_finding kinds (payload shapes; the table itself is unchanged):
--   alias_collision : { alias, a, b }
--     Two entities in the same resolution scope claim the same name/alias.
--   auto_alias      : { entity, alias, similarity, source }
--     An alias written by tier-4 fuzzy auto-resolve. Informational; undoable.
-- And `broken_wikilink` gains an optional `candidates: [{id, name, similarity}]`.
