DEFINE TABLE campaign SCHEMAFULL;
DEFINE FIELD name ON campaign TYPE string;
DEFINE FIELD system ON campaign TYPE string;
DEFINE FIELD created_at ON campaign TYPE datetime;
DEFINE FIELD updated_at ON campaign TYPE datetime;

DEFINE TABLE source SCHEMAFULL;
DEFINE FIELD campaign ON source TYPE record<campaign> | NULL;
DEFINE FIELD filename ON source TYPE string;
DEFINE FIELD display_name ON source TYPE string;
DEFINE FIELD source_type ON source TYPE string ASSERT $value IN ['rules', 'lore', 'supplement'];
DEFINE FIELD page_count ON source TYPE int;
DEFINE FIELD indexed_at ON source TYPE datetime;
DEFINE FIELD index_status ON source TYPE string ASSERT $value IN ['pending', 'indexing', 'done', 'error'];
DEFINE FIELD embed_model ON source TYPE string;

DEFINE TABLE chunk SCHEMAFULL;
DEFINE FIELD source ON chunk TYPE record<source>;
DEFINE FIELD campaign ON chunk TYPE record<campaign> | NULL;
DEFINE FIELD text ON chunk TYPE string;
DEFINE FIELD page_start ON chunk TYPE int;
DEFINE FIELD page_end ON chunk TYPE int;
DEFINE FIELD section_heading ON chunk TYPE string;
DEFINE FIELD source_type ON chunk TYPE string;
DEFINE FIELD embedding ON chunk TYPE array<float>;
DEFINE FIELD embed_model ON chunk TYPE string;

DEFINE INDEX chunk_embedding_idx ON chunk FIELDS embedding MTREE DIMENSION 768 DIST COSINE;

DEFINE TABLE entity SCHEMAFULL;
-- Phase 1: entity table exists but is minimal (no is_gm_only, no event/PC fields yet)
DEFINE FIELD campaign ON entity TYPE record<campaign>;
DEFINE FIELD entity_type ON entity TYPE string;
DEFINE FIELD name ON entity TYPE string;
DEFINE FIELD summary ON entity TYPE string;
DEFINE FIELD notes ON entity TYPE string;
DEFINE FIELD created_at ON entity TYPE datetime;
DEFINE FIELD updated_at ON entity TYPE datetime;

DEFINE TABLE session SCHEMAFULL;
DEFINE FIELD campaign ON session TYPE record<campaign>;
DEFINE FIELD session_number ON session TYPE int;
DEFINE FIELD title ON session TYPE string;
DEFINE FIELD date_played ON session TYPE string;
DEFINE FIELD notes ON session TYPE string;
DEFINE FIELD created_at ON session TYPE datetime;

DEFINE TABLE message SCHEMAFULL;
DEFINE FIELD campaign ON message TYPE record<campaign>;
DEFINE FIELD role ON message TYPE string ASSERT $value IN ['user', 'assistant', 'system'];
DEFINE FIELD content ON message TYPE string;
DEFINE FIELD citations ON message TYPE array<object>;
DEFINE FIELD created_at ON message TYPE datetime;

DEFINE TABLE relates_to TYPE RELATION FROM entity TO entity SCHEMAFULL;
DEFINE FIELD rel_type ON relates_to TYPE string;
DEFINE FIELD notes ON relates_to TYPE string;

DEFINE TABLE setting SCHEMAFULL;
DEFINE FIELD value ON setting TYPE string;
