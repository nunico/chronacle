DEFINE TABLE custom_provider SCHEMAFULL;
DEFINE FIELD name ON custom_provider TYPE string;
DEFINE FIELD provider_type ON custom_provider TYPE string ASSERT $value IN ['openai', 'anthropic'];
DEFINE FIELD base_url ON custom_provider TYPE string;
DEFINE FIELD api_key ON custom_provider TYPE string;
DEFINE FIELD created_at ON custom_provider TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON custom_provider TYPE datetime;
DEFINE INDEX unique_provider_name ON custom_provider FIELDS name UNIQUE;

DEFINE TABLE custom_provider_model SCHEMAFULL;
DEFINE FIELD provider ON custom_provider_model TYPE record<custom_provider>;
DEFINE FIELD model_id ON custom_provider_model TYPE string;
DEFINE FIELD display_name ON custom_provider_model TYPE string;
DEFINE FIELD created_at ON custom_provider_model TYPE datetime DEFAULT time::now();
DEFINE INDEX unique_model_per_provider ON custom_provider_model FIELDS provider, model_id UNIQUE;
