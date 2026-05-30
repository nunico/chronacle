# Custom LLM Providers — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow users to register custom LLM providers (OpenAI-compatible or Anthropic-compatible) with user-defined names and model lists, selectable from the settings UI.

**Architecture:** Two new SurrealDB tables (`custom_provider`, `custom_provider_model`) + a Rust service layer + 7 IPC commands + SettingsPage UI with management section and auto-switch.

**Tech Stack:** SurrealQL migrations, Rust (tokio + serde), Svelte 5, Tauri v2 IPC

---

## File Map

### New Files
| File | Responsibility |
|------|---------------|
| `src-tauri/src/schema/002_custom_providers.surql` | SurrealQL DDL for both tables + indexes |
| `src-tauri/src/services/custom_provider_service.rs` | CRUD functions using SurrealDB queries |

### Modified Files
| File | Changes |
|------|---------|
| `src-tauri/src/services/mod.rs` | Add `pub mod custom_provider_service;` |
| `src-tauri/src/providers/llm_provider.rs` | Add `AnthropicProvider::with_base_url()` constructor |
| `src-tauri/src/commands/mod.rs` | Add 7 new IPC commands, update imports |
| `src-tauri/src/lib.rs` | Register commands in `generate_handler![]`, update `build_llm_provider_from_map` |
| `src/lib/commands.ts` | Add 7 command wrappers + TypeScript interfaces |
| `src/SettingsPage.svelte` | Add custom providers section, model management, auto-switch logic |
| `src/app.css` | Add styles for new settings UI components |

---

### Task 1: Schema Migration

**Files:**
- Create: `src-tauri/src/schema/002_custom_providers.surql`

- [ ] **Step 1: Create migration file**

Write `src-tauri/src/schema/002_custom_providers.surql`:

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

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/schema/002_custom_providers.surql
git commit -m "feat(db): add custom_provider and custom_provider_model tables"
```

---

### Task 2: AnthropicProvider::with_base_url

**Files:**
- Modify: `src-tauri/src/providers/llm_provider.rs`

- [ ] **Step 1: Add `with_base_url` constructor to `AnthropicProvider`**

Add after the existing `impl AnthropicProvider` block, before the `#[async_trait] impl LlmProvider for AnthropicProvider` block:

```rust
impl AnthropicProvider {
    pub fn with_base_url(api_key: String, model: String, base_url: String) -> Self {
        let base = if base_url.is_empty() {
            "https://api.anthropic.com/v1".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        Self {
            api_key,
            model: if model.is_empty() {
                "claude-3-5-haiku-20241022".to_string()
            } else {
                model
            },
            base_url,
        }
    }
}
```

Add a `base_url: String` field to the `AnthropicProvider` struct.

Update the `chat_stream` method in the `#[async_trait] impl` block to use `self.base_url` instead of the hard-coded URL:

```rust
// Old:
let url = "https://api.anthropic.com/v1/messages".to_string();
// New:
let url = format!("{}/messages", self.base_url.trim_end_matches('/'));
```

- [ ] **Step 2: Update existing `AnthropicProvider` struct and `new()`**

The struct needs a `base_url` field. The `new()` constructor should set `base_url` to `"https://api.anthropic.com/v1"`.

- [ ] **Step 3: Verify compilation**

Run: `cargo build` (expected: compiles without errors)

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/providers/llm_provider.rs
git commit -m "feat: add AnthropicProvider::with_base_url for custom Anthropic-compatible endpoints"
```

---

### Task 3: Custom Provider Service

**Files:**
- Create: `src-tauri/src/services/custom_provider_service.rs`
- Modify: `src-tauri/src/services/mod.rs`

- [ ] **Step 1: Define structs at the top of the service file**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderRecord {
    pub id: surrealdb::sql::Thing,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CustomProvider {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
}

impl From<CustomProviderRecord> for CustomProvider {
    fn from(r: CustomProviderRecord) -> Self {
        Self {
            id: r.id.id.to_string(),
            name: r.name,
            provider_type: r.provider_type,
            base_url: r.base_url,
            api_key: r.api_key,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderModelRecord {
    pub id: surrealdb::sql::Thing,
    pub provider: surrealdb::sql::Thing,
    pub model_id: String,
    pub display_name: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CustomProviderModel {
    pub id: String,
    pub provider_id: String,
    pub model_id: String,
    pub display_name: String,
}

impl From<CustomProviderModelRecord> for CustomProviderModel {
    fn from(r: CustomProviderModelRecord) -> Self {
        Self {
            id: r.id.id.to_string(),
            provider_id: r.provider.id.to_string(),
            model_id: r.model_id,
            display_name: r.display_name,
        }
    }
}
```

- [ ] **Step 2: Implement CRUD functions**

```rust
use surrealdb::Surreal;
use surrealdb::engine::local::Db;

/// Get all custom providers, ordered by name.
pub async fn get_all(db: &Surreal<Db>) -> Result<Vec<CustomProvider>, String> {
    let mut response = db.query("SELECT * FROM custom_provider ORDER BY name ASC")
        .await
        .map_err(|e| format!("Failed to query custom providers: {e}"))?;
    let records: Vec<CustomProviderRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse custom providers: {e}"))?;
    Ok(records.into_iter().map(Into::into).collect())
}

/// Get a single custom provider by its record id (UUID string).
pub async fn get_by_id(db: &Surreal<Db>, id: &str) -> Result<CustomProvider, String> {
    let mut response = db.query("SELECT * FROM $id")
        .bind(("id", format!("custom_provider:{id}")))
        .await
        .map_err(|e| format!("Failed to query custom provider: {e}"))?;
    let record: Option<CustomProviderRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse custom provider: {e}"))?;
    record
        .map(Into::into)
        .ok_or_else(|| "Custom provider not found".to_string())
}

/// Create a new custom provider. Returns the created record.
pub async fn create(
    db: &Surreal<Db>,
    name: &str,
    provider_type: &str,
    base_url: &str,
    api_key: &str,
) -> Result<CustomProvider, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let mut response = db.query(
        "CREATE custom_provider SET
            id = $id,
            name = $name,
            provider_type = $provider_type,
            base_url = $base_url,
            api_key = $api_key,
            updated_at = time::now()"
    )
        .bind(("id", id.clone()))
        .bind(("name", name.to_owned()))
        .bind(("provider_type", provider_type.to_owned()))
        .bind(("base_url", base_url.to_owned()))
        .bind(("api_key", api_key.to_owned()))
        .await
        .map_err(|e| format!("Failed to create custom provider: {e}"))?;
    let created: Vec<CustomProviderRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse created provider: {e}"))?;
    created
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| "Failed to create custom provider: no record returned".to_string())
}

/// Update an existing custom provider. Fields that are empty strings are left unchanged.
pub async fn update(
    db: &Surreal<Db>,
    id: &str,
    name: &str,
    provider_type: &str,
    base_url: &str,
    api_key: &str,
) -> Result<CustomProvider, String> {
    let mut set_clauses = Vec::new();
    let safe_id = id.replace('`', "``");

    if !name.is_empty() {
        set_clauses.push(format!("name = '{}'", name.replace('\'', "''")));
    }
    if !provider_type.is_empty() {
        set_clauses.push(format!("provider_type = '{}'", provider_type.replace('\'', "''")));
    }
    if !base_url.is_empty() {
        set_clauses.push(format!("base_url = '{}'", base_url.replace('\'', "''")));
    }
    if !api_key.is_empty() {
        set_clauses.push(format!("api_key = '{}'", api_key.replace('\'', "''")));
    }
    set_clauses.push("updated_at = time::now()".to_string());

    let sql = format!(
        "UPDATE type::thing('custom_provider', '{safe_id}') SET {}",
        set_clauses.join(", ")
    );

    let mut response = db.query(sql)
        .await
        .map_err(|e| format!("Failed to update custom provider: {e}"))?;
    let updated: Vec<CustomProviderRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse updated provider: {e}"))?;
    updated
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| "Custom provider not found for update".to_string())
}

/// Delete a custom provider. Models are cascade-deleted by SurrealDB.
pub async fn delete(db: &Surreal<Db>, id: &str) -> Result<(), String> {
    let safe_id = id.replace('`', "``");
    db.query(format!("DELETE type::thing('custom_provider', '{safe_id}')"))
        .await
        .map_err(|e| format!("Failed to delete custom provider: {e}"))?;
    Ok(())
}
```

- [ ] **Step 3: Implement model functions**

```rust
/// Get all models for a custom provider, ordered by display_name.
pub async fn get_models(db: &Surreal<Db>, provider_id: &str) -> Result<Vec<CustomProviderModel>, String> {
    let safe_id = provider_id.replace('`', "``");
    let mut response = db.query(
        "SELECT * FROM custom_provider_model
         WHERE provider = type::thing('custom_provider', $id)
         ORDER BY display_name ASC"
    )
        .bind(("id", safe_id))
        .await
        .map_err(|e| format!("Failed to query provider models: {e}"))?;
    let records: Vec<CustomProviderModelRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse provider models: {e}"))?;
    Ok(records.into_iter().map(Into::into).collect())
}

/// Add a model to a custom provider.
pub async fn add_model(
    db: &Surreal<Db>,
    provider_id: &str,
    model_id: &str,
    display_name: &str,
) -> Result<CustomProviderModel, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let mut response = db.query(
        "CREATE custom_provider_model SET
            id = $id,
            provider = type::thing('custom_provider', $provider_id),
            model_id = $model_id,
            display_name = $display_name"
    )
        .bind(("id", id.clone()))
        .bind(("provider_id", provider_id.to_owned()))
        .bind(("model_id", model_id.to_owned()))
        .bind(("display_name", display_name.to_owned()))
        .await
        .map_err(|e| format!("Failed to add model: {e}"))?;
    let created: Vec<CustomProviderModelRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse created model: {e}"))?;
    created
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| "Failed to add model: no record returned".to_string())
}

/// Remove a model from a custom provider.
pub async fn remove_model(db: &Surreal<Db>, id: &str) -> Result<(), String> {
    let safe_id = id.replace('`', "``");
    db.query(format!("DELETE type::thing('custom_provider_model', '{safe_id}')"))
        .await
        .map_err(|e| format!("Failed to delete model: {e}"))?;
    Ok(())
}
```

- [ ] **Step 4: Add module to services/mod.rs**

Add `pub mod custom_provider_service;` to `src-tauri/src/services/mod.rs`.

- [ ] **Step 5: Verify compilation and commit**

```bash
cargo build
git add src-tauri/src/services/custom_provider_service.rs src-tauri/src/services/mod.rs
git commit -m "feat: add custom provider service with CRUD operations"
```

---

### Task 4: IPC Commands

**Files:**
- Modify: `src-tauri/src/commands/mod.rs`

- [ ] **Step 1: Add the 7 new command functions**

Add before the closing of the file (or after existing commands):

```rust
// ── Custom Provider Commands ──────────────────────────────────────────

#[tauri::command]
pub async fn get_custom_providers(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<CustomProviderResponse>, String> {
    let providers = crate::services::custom_provider_service::get_all(&state.db).await?;
    Ok(providers.into_iter().map(|p| CustomProviderResponse {
        id: p.id,
        name: p.name,
        provider_type: p.provider_type,
        base_url: p.base_url,
        api_key: p.api_key,
    }).collect())
}

#[tauri::command]
pub async fn create_custom_provider(
    state: State<'_, Arc<AppState>>,
    name: String,
    provider_type: String,
    base_url: String,
    api_key: String,
) -> Result<CustomProviderResponse, String> {
    if name.trim().is_empty() {
        return Err("Provider name is required".to_string());
    }
    if provider_type != "openai" && provider_type != "anthropic" {
        return Err("provider_type must be 'openai' or 'anthropic'".to_string());
    }
    if base_url.trim().is_empty() {
        return Err("Base URL is required".to_string());
    }
    let provider = crate::services::custom_provider_service::create(
        &state.db, name.trim(), &provider_type, base_url.trim(), &api_key,
    ).await?;
    Ok(CustomProviderResponse {
        id: provider.id,
        name: provider.name,
        provider_type: provider.provider_type,
        base_url: provider.base_url,
        api_key: provider.api_key,
    })
}

#[tauri::command]
pub async fn update_custom_provider(
    state: State<'_, Arc<AppState>>,
    id: String,
    name: String,
    provider_type: String,
    base_url: String,
    api_key: String,
) -> Result<CustomProviderResponse, String> {
    let provider = crate::services::custom_provider_service::update(
        &state.db, &id, &name, &provider_type, &base_url, &api_key,
    ).await?;
    Ok(CustomProviderResponse {
        id: provider.id,
        name: provider.name,
        provider_type: provider.provider_type,
        base_url: provider.base_url,
        api_key: provider.api_key,
    })
}

#[tauri::command]
pub async fn delete_custom_provider(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    crate::services::custom_provider_service::delete(&state.db, &id).await
}

#[tauri::command]
pub async fn get_provider_models(
    state: State<'_, Arc<AppState>>,
    provider_id: String,
) -> Result<Vec<ProviderModelResponse>, String> {
    let models = crate::services::custom_provider_service::get_models(&state.db, &provider_id).await?;
    Ok(models.into_iter().map(|m| ProviderModelResponse {
        id: m.id,
        provider_id: m.provider_id,
        model_id: m.model_id,
        display_name: m.display_name,
    }).collect())
}

#[tauri::command]
pub async fn add_provider_model(
    state: State<'_, Arc<AppState>>,
    provider_id: String,
    model_id: String,
    display_name: String,
) -> Result<ProviderModelResponse, String> {
    if model_id.trim().is_empty() {
        return Err("Model ID is required".to_string());
    }
    if display_name.trim().is_empty() {
        return Err("Display name is required".to_string());
    }
    let model = crate::services::custom_provider_service::add_model(
        &state.db, &provider_id, model_id.trim(), display_name.trim(),
    ).await?;
    Ok(ProviderModelResponse {
        id: model.id,
        provider_id: model.provider_id,
        model_id: model.model_id,
        display_name: model.display_name,
    })
}

#[tauri::command]
pub async fn remove_provider_model(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    crate::services::custom_provider_service::remove_model(&state.db, &id).await
}
```

- [ ] **Step 2: Add response structs at the top of commands/mod.rs**

Add after the existing `ChatMessageRow`, `ChatRequest`, etc.:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct CustomProviderResponse {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderModelResponse {
    pub id: String,
    pub provider_id: String,
    pub model_id: String,
    pub display_name: String,
}
```

- [ ] **Step 3: Add `use std::sync::Arc;` and `use tauri::State;` imports** (check if already present)

- [ ] **Step 4: Verify compilation and commit**

```bash
cargo build
git add src-tauri/src/commands/mod.rs
git commit -m "feat: add custom provider IPC commands"
```

---

### Task 5: Wire Up Backend (lib.rs)

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Register the 7 new commands in `generate_handler![]`**

Add these to the existing handler list:

```rust
commands::get_custom_providers,
commands::create_custom_provider,
commands::update_custom_provider,
commands::delete_custom_provider,
commands::get_provider_models,
commands::add_provider_model,
commands::remove_provider_model,
```

- [ ] **Step 2: Update `build_llm_provider_from_map` — make async and support custom providers**

Replace with:

```rust
pub(crate) async fn build_llm_provider_from_map(
    settings: &HashMap<String, String>,
    db: Option<&surrealdb::Surreal<surrealdb::engine::local::Db>>,
) -> Arc<dyn LlmProvider> {
    let provider = settings.get("llm_provider").map(|s| s.as_str()).unwrap_or("openai");
    let api_key = settings.get("llm_api_key").cloned().unwrap_or_default();
    let model = settings.get("llm_model").cloned().unwrap_or_default();
    let base_url = settings.get("llm_base_url").cloned().unwrap_or_default();

    // Check for custom provider prefix
    if let Some(custom_name) = provider.strip_prefix("custom:") {
        if let Some(db) = db {
            match build_custom_provider(db, custom_name, &model).await {
                Ok(p) => return p,
                Err(e) => {
                    eprintln!("Warning: custom provider '{custom_name}' not found ({e}), falling back to OpenAI");
                }
            }
        }
    }

    match provider {
        "anthropic" => Arc::new(AnthropicProvider::new(api_key, model)),
        "ollama" => Arc::new(OllamaProvider::new(base_url, model)),
        _ => Arc::new(OpenAIProvider::new(api_key, model)),
    }
}

async fn build_custom_provider(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    name: &str,
    model: &str,
) -> Result<Arc<dyn LlmProvider>, String> {
    let providers = crate::services::custom_provider_service::get_all(db).await?;

    let cp = providers.into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("Custom provider '{name}' not found"))?;

    match cp.provider_type.as_str() {
        "openai" => Ok(Arc::new(OpenAIProvider::with_base_url(
            cp.api_key, model.to_string(), cp.base_url,
        ))),
        "anthropic" => Ok(Arc::new(AnthropicProvider::with_base_url(
            cp.api_key, model.to_string(), cp.base_url,
        ))),
        _ => Err(format!("Unknown provider type: {}", cp.provider_type)),
    }
}
```

- [ ] **Step 3: Update `build_llm_provider_from_db` to pass `db` and await**

```rust
async fn build_llm_provider_from_db(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Arc<dyn LlmProvider> {
    let settings = match services::settings_service::get_all(db).await {
        Ok(s) => s.into_iter().map(|s| (s.key, s.value)).collect::<HashMap<_, _>>(),
        Err(_) => HashMap::new(),
    };

    build_llm_provider_from_map(&settings, Some(db)).await
}
```

- [ ] **Step 4: Update `reconfigure_llm_provider` command to pass db**

In `commands/mod.rs`, update the command to pass `Some(&state.db)`:

```rust
#[tauri::command]
pub async fn reconfigure_llm_provider(
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let settings = get_all_settings(&state.db).await?;
    let map: std::collections::HashMap<String, String> =
        settings.into_iter().map(|r| (r.id.id.to_string(), r.value)).collect();

    let new_provider = crate::build_llm_provider_from_map(&map, Some(&state.db)).await;
    let provider_type = crate::provider_type_name(&new_provider);

    *state
        .llm_provider
        .write()
        .map_err(|e| format!("Failed to acquire write lock: {e}"))? = new_provider;

    Ok(provider_type.to_string())
}
```

- [ ] **Step 5: Verify compilation and commit**

```bash
cargo build
git add src-tauri/src/lib.rs
git commit -m "feat: wire custom provider support into provider construction"
```

---

### Task 6: Frontend Command Wrappers

**Files:**
- Modify: `src/lib/commands.ts`

- [ ] **Step 1: Add TypeScript interfaces and command wrappers**

Add after the existing `LlmProviderStatus` interface:

```typescript
// ── Custom Provider Types ──────────────────────────────────────────────

export interface CustomProvider {
  id: string;
  name: string;
  provider_type: string;
  base_url: string;
  api_key: string;
}

export interface CustomProviderModel {
  id: string;
  provider_id: string;
  model_id: string;
  display_name: string;
}

export async function getCustomProviders(): Promise<CustomProvider[]> {
  return invoke<CustomProvider[]>('get_custom_providers');
}

export async function createCustomProvider(
  name: string,
  providerType: string,
  baseUrl: string,
  apiKey: string,
): Promise<CustomProvider> {
  return invoke<CustomProvider>('create_custom_provider', {
    name,
    providerType,
    baseUrl,
    apiKey,
  });
}

export async function updateCustomProvider(
  id: string,
  name: string,
  providerType: string,
  baseUrl: string,
  apiKey: string,
): Promise<CustomProvider> {
  return invoke<CustomProvider>('update_custom_provider', {
    id,
    name,
    providerType,
    baseUrl,
    apiKey,
  });
}

export async function deleteCustomProvider(id: string): Promise<void> {
  return invoke('delete_custom_provider', { id });
}

export async function getProviderModels(providerId: string): Promise<CustomProviderModel[]> {
  return invoke<CustomProviderModel[]>('get_provider_models', { providerId });
}

export async function addProviderModel(
  providerId: string,
  modelId: string,
  displayName: string,
): Promise<CustomProviderModel> {
  return invoke<CustomProviderModel>('add_provider_model', {
    providerId,
    modelId,
    displayName,
  });
}

export async function removeProviderModel(id: string): Promise<void> {
  return invoke('remove_provider_model', { id });
}
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/commands.ts
git commit -m "feat: add custom provider command wrappers to frontend"
```

---

### Task 7: SettingsPage UI — Custom Provider Section

**Files:**
- Modify: `src/SettingsPage.svelte`
- Modify: `src/app.css` (if needed)

- [ ] **Step 1: Update script section to load custom providers**

Add these imports at the top of `<script lang="ts">`:

```typescript
import {
  getCustomProviders,
  createCustomProvider,
  updateCustomProvider,
  deleteCustomProvider,
  getProviderModels,
  addProviderModel,
  removeProviderModel,
  type CustomProvider,
  type CustomProviderModel,
} from './lib/commands';
```

Add state variables:

```typescript
let customProviders = $state<CustomProvider[]>([]);
let providerModels = $state<Map<string, CustomProviderModel[]>>(new Map());
let editingProvider = $state<CustomProvider | null>(null);
let showAddProvider = $state(false);
let newProviderName = $state('');
let newProviderType = $state('openai');
let newProviderBaseUrl = $state('');
let newProviderApiKey = $state('');
let editingProviderModels = $state<string | null>(null); // provider id
let newModelId = $state('');
let newModelDisplayName = $state('');
```

Add load function:

```typescript
async function loadCustomProviders() {
  try {
    const providers = await getCustomProviders();
    customProviders = providers;
    // Load models for each provider
    const modelsMap = new Map<string, CustomProviderModel[]>();
    for (const p of providers) {
      const models = await getProviderModels(p.id);
      modelsMap.set(p.id, models);
    }
    providerModels = modelsMap;
  } catch (e) {
    console.error('Failed to load custom providers:', e);
  }
}
```

Update `onMount` to also call `loadCustomProviders()`.

- [ ] **Step 2: Add custom provider form handlers**

```typescript
async function handleAddProvider() {
  if (!newProviderName.trim() || !newProviderBaseUrl.trim()) return;
  try {
    await createCustomProvider(
      newProviderName.trim(),
      newProviderType,
      newProviderBaseUrl.trim(),
      newProviderApiKey,
    );
    newProviderName = '';
    newProviderType = 'openai';
    newProviderBaseUrl = '';
    newProviderApiKey = '';
    showAddProvider = false;
    await loadCustomProviders();
  } catch (e) {
    showError(`Failed to create provider: ${e}`);
  }
}

async function handleDeleteProvider(id: string) {
  try {
    await deleteCustomProvider(id);
    await loadCustomProviders();
  } catch (e) {
    showError(`Failed to delete provider: ${e}`);
  }
}

async function handleAddModel(providerId: string) {
  if (!newModelId.trim() || !newModelDisplayName.trim()) return;
  try {
    await addProviderModel(providerId, newModelId.trim(), newModelDisplayName.trim());
    newModelId = '';
    newModelDisplayName = '';
    // Reload models for this provider
    const models = await getProviderModels(providerId);
    providerModels.set(providerId, models);
    providerModels = new Map(providerModels);
  } catch (e) {
    showError(`Failed to add model: ${e}`);
  }
}

async function handleRemoveModel(id: string, providerId: string) {
  try {
    await removeProviderModel(id);
    const models = await getProviderModels(providerId);
    providerModels.set(providerId, models);
    providerModels = new Map(providerModels);
  } catch (e) {
    showError(`Failed to remove model: ${e}`);
  }
}
```

- [ ] **Step 3: Add "Custom" tab/button alongside Chat/Settings in App.svelte**

Actually — the custom providers section goes inside the SettingsPage, not as its own page. The existing settings layout already has provider config. The custom providers management section can be added as a section below the existing provider config.

Add the following HTML inside `<div class="settings-page">`, after the existing `.config-section` for LLM provider:

```html
<hr />

<section class="config-section custom-providers-section">
  <h3>Custom Providers</h3>
  <p class="hint">Register API-compatible providers (OpenRouter, Groq, etc.)</p>

  {#if customProviders.length === 0 && !showAddProvider}
    <p class="empty-state">No custom providers configured yet.</p>
  {/if}

  {#each customProviders as cp (cp.id)}
    <div class="custom-provider-card">
      <div class="provider-header">
        <strong>{cp.name}</strong>
        <span class="type-badge">{cp.provider_type === 'openai' ? 'OpenAI-compatible' : 'Anthropic-compatible'}</span>
        <button class="small-btn" onclick={() => { editingProvider = cp; }}>Edit</button>
        <button class="small-btn danger" onclick={() => handleDeleteProvider(cp.id)}>Delete</button>
      </div>
      <div class="provider-detail">
        <span class="label">Base URL:</span>
        <code>{cp.base_url}</code>
      </div>
      <div class="provider-detail">
        <span class="label">Models:</span>
        {#if (providerModels.get(cp.id)?.length ?? 0) === 0}
          <span class="text-muted">No models added</span>
        {:else}
          <ul class="model-list">
            {#each providerModels.get(cp.id) ?? [] as model (model.id)}
              <li>
                <span class="model-display">{model.display_name}</span>
                <code class="model-id">{model.model_id}</code>
                <button class="small-btn danger" onclick={() => handleRemoveModel(model.id, cp.id)}>×</button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>

      {#if editingProviderModels === cp.id}
        <div class="add-model-form">
          <input
            type="text"
            placeholder="Model ID (e.g. gpt-4o)"
            bind:value={newModelId}
          />
          <input
            type="text"
            placeholder="Display name (e.g. GPT-4o)"
            bind:value={newModelDisplayName}
          />
          <button class="small-btn primary" onclick={() => handleAddModel(cp.id)}>Add</button>
        </div>
      {/if}
      <button
        class="small-btn"
        onclick={() => {
          editingProviderModels = editingProviderModels === cp.id ? null : cp.id;
          newModelId = '';
          newModelDisplayName = '';
        }}
      >
        {editingProviderModels === cp.id ? 'Cancel' : '+ Add Model'}
      </button>
    </div>
  {/each}

  {#if showAddProvider}
    <div class="add-provider-form">
      <label for="new-provider-name">Provider Name</label>
      <input id="new-provider-name" type="text" bind:value={newProviderName} placeholder="e.g. OpenRouter" />

      <label for="new-provider-type">API Compatibility</label>
      <select id="new-provider-type" bind:value={newProviderType}>
        <option value="openai">OpenAI-compatible</option>
        <option value="anthropic">Anthropic-compatible</option>
      </select>

      <label for="new-provider-url">Base URL</label>
      <input id="new-provider-url" type="text" bind:value={newProviderBaseUrl} placeholder="https://openrouter.ai/api/v1" />

      <label for="new-provider-key">API Key (optional)</label>
      <input id="new-provider-key" type="password" bind:value={newProviderApiKey} autocomplete="off" />

      <div class="form-actions">
        <button onclick={() => { showAddProvider = false; }}>Cancel</button>
        <button class="primary" onclick={handleAddProvider}>Save Provider</button>
      </div>
    </div>
  {:else}
    <button class="small-btn primary" onclick={() => { showAddProvider = true; }}>+ Add Custom Provider</button>
  {/if}
</section>
```

- [ ] **Step 4: Add CSS styles in the `<style>` section**

```css
.custom-providers-section {
  margin-top: 1.5rem;
}

.custom-provider-card {
  background: var(--bg-assistant);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 0.75rem;
  margin-bottom: 0.75rem;
}

.provider-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.5rem;
}

.provider-header strong {
  font-size: 0.95rem;
}

.type-badge {
  font-size: 0.7rem;
  background: var(--bg-user);
  color: var(--text-muted);
  padding: 0.15rem 0.4rem;
  border-radius: 3px;
}

.provider-detail {
  font-size: 0.85rem;
  margin-bottom: 0.3rem;
}

.provider-detail .label {
  color: var(--text-muted);
  margin-right: 0.25rem;
}

.provider-detail code {
  font-size: 0.8rem;
  color: var(--text-muted);
  background: var(--bg-input);
  padding: 0.1rem 0.3rem;
  border-radius: 3px;
}

.model-list {
  list-style: none;
  padding: 0;
  margin: 0.3rem 0;
}

.model-list li {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.2rem 0;
  font-size: 0.85rem;
}

.model-display {
  font-weight: 500;
}

.model-id {
  font-size: 0.75rem;
  color: var(--text-muted);
}

.small-btn {
  background: none;
  border: 1px solid var(--border);
  border-radius: 3px;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 0.75rem;
  padding: 0.2rem 0.5rem;
  font-family: inherit;
}

.small-btn:hover {
  background: var(--bg-user);
  color: var(--text);
}

.small-btn.danger {
  color: #fca5a5;
  border-color: #7f1d1d;
}

.small-btn.danger:hover {
  background: #7f1d1d;
}

.small-btn.primary {
  background: var(--accent);
  color: #fff;
  border-color: var(--accent);
}

.add-provider-form {
  background: var(--bg-surface);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 1rem;
  margin-bottom: 0.75rem;
}

.add-model-form {
  display: flex;
  gap: 0.3rem;
  margin: 0.5rem 0;
  align-items: center;
}

.add-model-form input {
  flex: 1;
  padding: 0.3rem 0.5rem;
  border: 1px solid var(--border);
  border-radius: 3px;
  background: var(--bg-input);
  color: var(--text);
  font-family: inherit;
  font-size: 0.85rem;
}

.form-actions {
  display: flex;
  gap: 0.5rem;
  margin-top: 0.75rem;
}

.empty-state {
  color: var(--text-muted);
  font-size: 0.85rem;
  text-align: center;
  padding: 1rem;
}

.text-muted {
  color: var(--text-muted);
  font-size: 0.85rem;
}

hr {
  border: none;
  border-top: 1px solid var(--border);
  margin: 1.5rem 0;
}
```

- [ ] **Step 5: Commit**

```bash
git add src/SettingsPage.svelte src/app.css
git commit -m "feat: add custom providers management section to settings UI"
```

---

### Task 8: SettingsPage UI — Auto-Switch Provider Selection

**Files:**
- Modify: `src/SettingsPage.svelte`

- [ ] **Step 1: Compute merged provider options**

Add a derived state that merges built-in types with custom providers:

```typescript
let providerOptions = $derived.by(() => {
  const builtin = [
    { value: 'openai', label: 'OpenAI' },
    { value: 'anthropic', label: 'Anthropic' },
    { value: 'ollama', label: 'Ollama (Local)' },
  ];
  const custom = customProviders.map(cp => ({
    value: `custom:${cp.name}`,
    label: `Custom: ${cp.name}`,
  }));
  return custom.length > 0
    ? [...builtin, { value: '', label: '──────────', disabled: true }, ...custom]
    : builtin;
});
```

- [ ] **Step 2: Update provider dropdown to use computed options**

Replace the static `<select>` for provider with:

```html
<select id="provider" bind:value={providerType}>
  {#each providerOptions as opt (opt.value)}
    <option value={opt.value} disabled={opt.disabled}>{opt.label}</option>
  {/each}
</select>
```

- [ ] **Step 3: Add auto-populate logic when a custom provider is selected**

```typescript
// In the script section, add a reactive effect for providerType changes
$effect(() => {
  if (providerType.startsWith('custom:')) {
    const name = providerType.slice('custom:'.length);
    const cp = customProviders.find(p => p.name === name);
    if (cp) {
      apiKey = cp.api_key;
      baseUrl = cp.base_url;
      // Clear model so user can pick from the dropdown or type
      model = '';
    }
  }
});
```

- [ ] **Step 4: Update the model dropdown for custom providers**

Replace the model input with a conditional dropdown when a custom provider is selected:

```html
{#if providerType.startsWith('custom:')}
  <label for="model">Model</label>
  <select id="model" bind:value={model}>
    {#if !model}<option value="">Select a model…</option>{/if}
    {#each providerModels.get(selectedCustomProviderId ?? '') ?? [] as cm (cm.id)}
      <option value={cm.model_id}>{cm.display_name}</option>
    {/each}
  </select>
{:else}
  <!-- existing model input -->
  <label for="model">Model</label>
  <input id="model" type="text" bind:value={model} placeholder={modelPlaceholder} />
{/if}
```

Using a `<select>` for models requires a new derived to get the current custom provider's id:

```typescript
let selectedCustomProviderId = $derived.by(() => {
  if (!providerType.startsWith('custom:')) return null;
  const name = providerType.slice('custom:'.length);
  return customProviders.find(p => p.name === name)?.id ?? null;
});
```

- [ ] **Step 5: Commit**

```bash
git add src/SettingsPage.svelte
git commit -m "feat: add auto-switch to custom provider in provider dropdown"
```

---

### Task 9: Integration Tests

**Files:**
- Modify: `src-tauri/tests/integration_test.rs`

- [ ] **Step 1: Add test for full custom provider lifecycle**

Append to the test file:

```rust
#[tokio::test]
async fn test_custom_provider_crud() {
    let db = new_db().await;

    // Create a custom provider
    let created = crate::services::custom_provider_service::create(
        &db, "TestProvider", "openai",
        "https://test.api.com/v1", "sk-test-123",
    ).await.expect("create should succeed");
    assert_eq!(created.name, "TestProvider");
    assert_eq!(created.provider_type, "openai");

    // Add models
    let model1 = crate::services::custom_provider_service::add_model(
        &db, &created.id, "gpt-4o", "GPT-4o",
    ).await.expect("add model should succeed");
    assert_eq!(model1.model_id, "gpt-4o");
    assert_eq!(model1.display_name, "GPT-4o");

    let _model2 = crate::services::custom_provider_service::add_model(
        &db, &created.id, "claude-3-haiku", "Claude 3 Haiku",
    ).await.expect("add model should succeed");

    // Get models
    let models = crate::services::custom_provider_service::get_models(&db, &created.id)
        .await.expect("get models should succeed");
    assert_eq!(models.len(), 2);

    // Get all providers
    let all = crate::services::custom_provider_service::get_all(&db)
        .await.expect("get all should succeed");
    assert!(!all.is_empty());

    // Delete a model
    crate::services::custom_provider_service::remove_model(&db, &model1.id)
        .await.expect("remove model should succeed");
    let models_after = crate::services::custom_provider_service::get_models(&db, &created.id)
        .await.expect("get models after delete should succeed");
    assert_eq!(models_after.len(), 1);

    // Delete the provider
    crate::services::custom_provider_service::delete(&db, &created.id)
        .await.expect("delete should succeed");
    let after_delete = crate::services::custom_provider_service::get_all(&db)
        .await.expect("get all after delete should succeed");
    assert!(after_delete.iter().all(|p| p.id != created.id), "provider should be deleted");

    // Models should also be gone (cascade delete)
    let models_final = crate::services::custom_provider_service::get_models(&db, &created.id)
        .await.expect("get models after provider delete should succeed");
    assert!(models_final.is_empty(), "models should cascade-delete");
}

#[tokio::test]
async fn test_custom_provider_duplicate_name() {
    let db = new_db().await;

    crate::services::custom_provider_service::create(
        &db, "Duplicate", "openai",
        "https://api1.com", "key1",
    ).await.expect("first create should succeed");

    let result = crate::services::custom_provider_service::create(
        &db, "Duplicate", "anthropic",
        "https://api2.com", "key2",
    ).await;
    assert!(result.is_err(), "duplicate name should fail");
}

#[tokio::test]
async fn test_custom_provider_update() {
    let db = new_db().await;

    let created = crate::services::custom_provider_service::create(
        &db, "UpdateMe", "openai",
        "https://old.api.com", "old-key",
    ).await.expect("create should succeed");

    let updated = crate::services::custom_provider_service::update(
        &db, &created.id, "UpdatedName", "anthropic",
        "https://new.api.com", "new-key",
    ).await.expect("update should succeed");

    assert_eq!(updated.name, "UpdatedName");
    assert_eq!(updated.provider_type, "anthropic");
    assert_eq!(updated.base_url, "https://new.api.com");
    assert_eq!(updated.api_key, "new-key");
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test test_custom_provider -- --nocapture
```
Expected: all pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tests/integration_test.rs
git commit -m "test: add custom provider CRUD integration tests"
```

---

## Self-Check

| Spec Requirement | Task | Status |
|-----------------|------|--------|
| Schema migration (2 tables, fields, indexes) | Task 1 | ✓ |
| AnthropicProvider::with_base_url | Task 2 | ✓ |
| CRUD service for custom providers | Task 3 | ✓ |
| CRUD service for provider models | Task 3 | ✓ |
| 7 IPC commands | Task 4 | ✓ |
| Update build_llm_provider_from_map | Task 5 | ✓ |
| Frontend command wrappers | Task 6 | ✓ |
| Settings page custom providers section | Task 7 | ✓ |
| Provider dropdown with custom entries | Task 8 | ✓ |
| Auto-switch (selected custom provider fills key/URL/models) | Task 8 | ✓ |
| Error handling (duplicate name, empty fields, cascade delete) | Task 1 (DB), Task 3, Task 4 | ✓ |
| Integration tests | Task 9 | ✓ |
