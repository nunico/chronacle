-- 007_session_embedding.surql
-- Make session notes retrievable: add embedding fields + MTREE index to the
-- session table, mirroring the entity-table pattern from migration 006.
-- Session notes are embedded (title + notes) on create/update by
-- `session_service::embed_session` and KNN-searched in
-- `agent_service::fetch_entity_context`.

DEFINE FIELD embedding   ON TABLE session TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD embed_model ON TABLE session TYPE string | NULL DEFAULT NULL;

-- Cosine MTREE, 768-dim, matches nomic-embed-text-v1.5 (same as chunk/entities).
DEFINE INDEX session_embedding_idx ON TABLE session FIELDS embedding MTREE DIMENSION 768 DIST COSINE;
