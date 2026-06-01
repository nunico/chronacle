-- 003_collections.surql
-- Introduces the collection table, subscribes_to relation, and
-- collection fields on source/chunk. The old campaign fields on
-- source/chunk remain defined (dead) for schema idempotency.

DEFINE TABLE collection SCHEMAFULL;
DEFINE FIELD name        ON collection TYPE string;
DEFINE FIELD description ON collection TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at  ON collection TYPE datetime;
DEFINE FIELD updated_at  ON collection TYPE datetime;

DEFINE TABLE subscribes_to SCHEMAFULL TYPE RELATION FROM campaign TO collection;
DEFINE FIELD created_at ON subscribes_to TYPE datetime;
DEFINE INDEX unique_subscription ON TABLE subscribes_to COLUMNS in, out UNIQUE;

-- Add collection field to source (non-nullable — service layer enforces required at insert time).
-- The old campaign field remains defined but is never set by new code.
DEFINE FIELD collection ON source TYPE record<collection>;

-- Add collection field to chunk (inherited from source at index time).
DEFINE FIELD collection ON chunk TYPE record<collection>;
