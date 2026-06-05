-- 005_session_update.surql
-- Adds updated_at field and campaign index to the session table.
-- updated_at was missing from the Phase 1 definition in 001_initial.surql.
-- idx_session_campaign supports efficient lookup of sessions by campaign.

DEFINE FIELD updated_at ON session TYPE datetime DEFAULT time::now();

DEFINE INDEX idx_session_campaign ON session FIELDS campaign;
