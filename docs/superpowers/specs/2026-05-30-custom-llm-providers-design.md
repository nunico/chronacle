# Custom LLM Providers — Design Spec

**Date:** 2026-05-30
**Status:** Approved
**Phase:** 1.5 (post-core-RAG)

---

## 1. Motivation

Users want to use third-party LLM services that are API-compatible with OpenAI's or Anthropic's wire format (e.g. OpenRouter, Groq, Together AI, Fireworks, Azure OpenAI). Currently the app only supports the official OpenAI, Anthropic, and Ollama endpoints with hard-coded URLs.

This feature allows users to register arbitrary "custom providers" with a name, API type, base URL, API key, and an associated list of models.

---

## 2. Data Model

### New tables

```surql
DEFINE TABLE custom_provider SCHEMAFULL;
DEFINE FIELD name ON custom_provider TYPE string;
DEFINE FIELD provider_type ON custom_provider TYPE string ASSERT $value IN ["openai", "anthropic"];
DEFINE FIELD base_url ON custom_provider TYPE string;
DEFINE FIELD api_key ON custom_provider TYPE string;
DEFINE FIELD created_at ON custom_provider TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON custom_provider TYPE datetime;
DEFINE INDEX unique_provider_name ON custom_provider FIELDS name UNIQUE;

DEFINE TABLE custom_provider_model SCHEMAFULL;
DEFINE FIELD provider ON custom_provider_model TYPE record(custom_provider);
DEFINE FIELD model_id ON custom_provider_model TYPE string;
DEFINE FIELD display_name ON custom_provider_model TYPE string;
DEFINE FIELD created_at ON custom_provider_model TYPE datetime DEFAULT time::now();
DEFINE INDEX idx_model_provider ON custom_provider_model FIELDS provider;
```

### Structs (Rust)

```rust
pub struct CustomProvider {
    pub id: String,
    pub name: String,
    pub provider_type: String,   // "openai" | "anthropic"
    pub base_url: String,
    pub api_key: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

pub struct CustomProviderModel {
    pub id: String,
    pub provider: String,        // record link e.g. "custom_provider:abc123"
    pub model_id: String,        // wire-format model name
    pub display_name: String,    // user-facing label
    pub created_at: Option<DateTime<Utc>>,
}
```

---

## 3. Rust Backend

### New file: `src-tauri/src/services/custom_provider_service.rs`

CRUD service with SurrealQL queries:

| Function | SurrealQL Pattern |
|----------|-------------------|
| `get_all(db)` | `SELECT * FROM custom_provider ORDER BY name ASC` |
| `get_by_id(db, id)` | `SELECT * FROM type::thing('custom_provider', $id)` |
| `create(db, params)` | `CREATE custom_provider SET name=$n, provider_type=$t, base_url=$b, api_key=$k, updated_at=time::now()` |
| `update(db, id, params)` | `UPDATE type::thing('custom_provider', $id) MERGE { ... }` |
| `delete(db, id)` | `DELETE type::thing('custom_provider', $id)` (SurrealDB cascades via schema) |

Models:

| Function | SurrealQL Pattern |
|----------|-------------------|
| `get_models(db, provider_id)` | `SELECT * FROM custom_provider_model WHERE provider = type::thing('custom_provider', $id) ORDER BY display_name ASC` |
| `add_model(db, params)` | `CREATE custom_provider_model SET provider=$p, model_id=$m, display_name=$d` |
| `remove_model(db, id)` | `DELETE type::thing('custom_provider_model', $id)` |

### New IPC commands (in `commands/mod.rs`)

| Command | Signature | Returns |
|---------|-----------|---------|
| `get_custom_providers` | `(state)` | `Vec<CustomProvider>` |
| `create_custom_provider` | `(state, name, provider_type, base_url, api_key)` | `CustomProvider` |
| `update_custom_provider` | `(state, id, name, provider_type, base_url, api_key)` | `CustomProvider` |
| `delete_custom_provider` | `(state, id)` | `()` |
| `get_provider_models` | `(state, provider_id)` | `Vec<CustomProviderModel>` |
| `add_provider_model` | `(state, provider_id, model_id, display_name)` | `CustomProviderModel` |
| `remove_provider_model` | `(state, id)` | `()` |

All commands validate that string fields are non-empty (except `api_key` — empty means "no key configured").

### Modified `build_llm_provider_from_map`

When `llm_provider` starts with `"custom:"`, extract the provider name, query the DB, and construct:

- `provider_type == "openai"` → `OpenAIProvider::with_base_url(api_key, model, base_url)`
- `provider_type == "anthropic"` → `AnthropicProvider::with_base_url(api_key, model, base_url)`

This requires adding a `with_base_url` constructor to `AnthropicProvider`.

### Modified `provider_type_name`

For custom providers, return the stored `name` instead of a type constant, so the UI shows "OpenRouter" rather than "openai".

---

## 4. Frontend UI

### Settings Page Layout

The settings page gains a two-column layout (on wide windows):

**Left** — Built-in provider config (existing, minor changes):
- Provider dropdown includes built-in types (OpenAI, Anthropic, Ollama) plus a separator group with `Custom: {name}` entries
- Selecting `Custom: {name}` auto-populates api_key, base_url from the custom provider record
- Model dropdown populates from that provider's model list

**Right** — Custom Providers management section:
- List of registered custom providers with name, type badge, model count, and Edit/Delete actions
- **+ Add Provider** button → inline form (name, API compatibility, base URL, API key)
- Each provider expands to show its model list with Add/Remove controls

### Auto-Switch Flow

1. User selects `Custom: OpenRouter` in provider dropdown
2. `llm_provider` setting → `"custom:OpenRouter"`
3. `llm_api_key` and `llm_base_url` settings → values from the custom provider record
4. Model dropdown loads models from `get_provider_models(provider_id)`
5. On "Save & Connect", `reconfigure_llm_provider` reads `llm_provider` prefix → queries DB → constructs provider

If the custom provider is deleted while active, startup falls back to built-in `openai` with a warning.

### New commands in `src/lib/commands.ts`

```typescript
export interface CustomProvider {
  id: string;
  name: string;
  provider_type: string;
  base_url: string;
  api_key: string;
}

export function getCustomProviders(): Promise<CustomProvider[]>;
export function createCustomProvider(name, type, baseUrl, apiKey): Promise<CustomProvider>;
export function updateCustomProvider(id, name, type, baseUrl, apiKey): Promise<CustomProvider>;
export function deleteCustomProvider(id): Promise<void>;
export function getProviderModels(providerId): Promise<CustomProviderModel[]>;
export function addProviderModel(providerId, modelId, displayName): Promise<CustomProviderModel>;
export function removeProviderModel(id): Promise<void>;
```

---

## 5. Migration

New file: `src-tauri/src/schema/002_custom_providers.surql`

Contains the `DEFINE TABLE` / `DEFINE FIELD` / `DEFINE INDEX` statements for both tables.

`schema::run_migrations` already iterates over `*.surql` files in order, so no changes needed to the migration runner.

---

## 6. Error Handling

| Scenario | Handle |
|----------|--------|
| Duplicate provider name | Unique index on `name` → DB returns error → surfaced to UI |
| Missing/invalid fields | Frontend validation (non-empty name, valid URL prefix) + backend validation |
| Deleted provider still active | Startup fallback to `openai`; runtime `reconfigure_llm_provider` returns error |
| Empty API key | Config error at invocation time (existing pattern) |
| Provider type not `openai`/`anthropic` | Rejected by DB `ASSERT` clause |

---

## 7. Testing

- **Unit** (service layer with mock DB): CRUD, cascade delete, duplicate name, empty field rejection
- **Integration** (SurrealDB mem): full flow — create provider → add models → read back → delete → verify cascade
- **Frontend** (Vitest + MSW): provider dropdown rendering with custom entries, model add/remove actions

---

## 8. Files Changed / Created

| File | Action |
|------|--------|
| `src-tauri/src/schema/002_custom_providers.surql` | Create |
| `src-tauri/src/services/custom_provider_service.rs` | Create |
| `src-tauri/src/services/mod.rs` | Edit (add module) |
| `src-tauri/src/commands/mod.rs` | Edit (add 7 commands) |
| `src-tauri/src/lib.rs` | Edit (register commands, update build_llm_provider_from_map) |
| `src-tauri/src/providers/llm_provider.rs` | Edit (add AnthropicProvider::with_base_url) |
| `src/lib/commands.ts` | Edit (add 7 command wrappers) |
| `src/SettingsPage.svelte` | Edit (add custom providers section, model management, auto-switch) |
| `src/app.css` | Maybe (add styles for new components) |