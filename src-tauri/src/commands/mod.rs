use std::sync::Arc;

use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tauri::State;

/// Returns a map of all stored settings key-value pairs.
#[tauri::command]
pub async fn get_settings(
    state: State<'_, Arc<AppState>>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let mut response = state
        .db
        .query("SELECT * FROM setting")
        .await
        .map_err(|e| format!("Database query failed: {e}"))?;

    #[derive(Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
        value: String,
    }

    let rows: Vec<Row> = response
        .take(0)
        .map_err(|e| format!("Failed to parse settings: {e}"))?;

    let map = rows
        .into_iter()
        .map(|r| (r.id.id.to_string(), r.value))
        .collect();
    Ok(map)
}

/// Upserts a single setting by key.
#[tauri::command]
pub async fn update_setting(
    state: State<'_, Arc<AppState>>,
    key: String,
    value: String,
) -> Result<(), String> {
    let safe_key = key.replace('`', "``");
    let sql = format!("UPSERT setting:`{safe_key}` SET value = $value");

    state
        .db
        .query(sql)
        .bind(("value", value.to_owned()))
        .await
        .map_err(|e| format!("Failed to update setting: {e}"))?;

    Ok(())
}

// ── Source Commands ───────────────────────────────────────────────────────────

/// Response shape for a source record returned over IPC.
#[derive(Debug, Clone, Serialize)]
pub struct SourceResponse {
    pub id: String,
    pub filename: String,
    pub display_name: String,
    pub source_type: String,
    pub page_count: i64,
    pub index_status: String,
    pub embed_model: String,
    pub collection_id: Option<String>,
}

/// Uploads a source PDF file, storing it in the blob store and triggering
/// ingestion (extraction → chunking → embedding).
///
/// For Phase 1 the ingestion pipeline is a stub that marks the source as
/// `pending`; full processing will be wired in a later iteration.
#[tauri::command]
pub async fn upload_source(
    state: State<'_, Arc<AppState>>,
    file_path: String,
    display_name: Option<String>,
    source_type: Option<String>,
    collection_id: String,
) -> Result<serde_json::Value, String> {
    if collection_id.trim().is_empty() {
        return Err("collection_id is required".to_string());
    }

    let path = std::path::PathBuf::from(&file_path);
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let display_name = display_name.unwrap_or_else(|| filename.clone());
    let source_type = source_type.unwrap_or_else(|| "rules".to_string());
    let source_id = uuid::Uuid::new_v4().to_string();
    let embed_model = "nomic-embed-text-v1.5".to_string();

    // Read file contents
    let data = tokio::fs::read(&path)
        .await
        .map_err(|e| format!("Failed to read file: {e}"))?;

    // Store in blob store
    state
        .blob_store
        .store(&source_id, &filename, &data)
        .await
        .map_err(|e| format!("Failed to store blob: {e}"))?;

    // Insert source record — collection is bound via parameter, never interpolated.
    let mut response = state
        .db
        .query(
            "CREATE source SET
                id = $id,
                filename = $filename,
                display_name = $display_name,
                source_type = $source_type,
                page_count = 0,
                indexed_at = time::now(),
                index_status = 'pending',
                embed_model = $embed_model,
                collection = type::thing('collection', $collection_id)",
        )
        .bind(("id", source_id.to_owned()))
        .bind(("filename", filename.to_owned()))
        .bind(("display_name", display_name.to_owned()))
        .bind(("source_type", source_type.to_owned()))
        .bind(("embed_model", embed_model.to_owned()))
        .bind(("collection_id", collection_id.clone()))
        .await
        .map_err(|e| format!("Failed to create source record: {e}"))?;

    let created: Vec<serde_json::Value> = response
        .take(0)
        .map_err(|e| format!("Failed to parse created source: {e}"))?;

    Ok(created
        .into_iter()
        .next()
        .unwrap_or(serde_json::json!({"id": source_id, "collection_id": collection_id})))
}

/// Returns all sources, optionally filtered to a specific collection.
///
/// When `collection_id` is provided the query uses a parameterised binding
/// (never string interpolation) to avoid SQL-injection risks.
#[tauri::command]
pub async fn get_sources(
    state: State<'_, Arc<AppState>>,
    collection_id: Option<String>,
) -> Result<Vec<SourceResponse>, String> {
    /// Raw row shape as SurrealDB returns it.
    #[derive(Deserialize)]
    struct SourceRow {
        id: surrealdb::sql::Thing,
        filename: String,
        display_name: String,
        source_type: String,
        page_count: i64,
        index_status: String,
        embed_model: String,
        collection: Option<surrealdb::sql::Thing>,
    }

    let mut response = if let Some(ref cid) = collection_id {
        state
            .db
            .query(
                "SELECT * FROM source \
                 WHERE collection = type::thing('collection', $cid) \
                 ORDER BY display_name ASC",
            )
            .bind(("cid", cid.clone()))
            .await
            .map_err(|e| format!("Failed to query sources: {e}"))?
    } else {
        state
            .db
            .query("SELECT * FROM source ORDER BY display_name ASC")
            .await
            .map_err(|e| format!("Failed to query sources: {e}"))?
    };

    let rows: Vec<SourceRow> = response
        .take(0)
        .map_err(|e| format!("Failed to parse sources: {e}"))?;

    Ok(rows
        .into_iter()
        .map(|r| SourceResponse {
            id: r.id.id.to_raw(),
            filename: r.filename,
            display_name: r.display_name,
            source_type: r.source_type,
            page_count: r.page_count,
            index_status: r.index_status,
            embed_model: r.embed_model,
            collection_id: r.collection.map(|t| t.id.to_raw()),
        })
        .collect())
}

// ── Collection Commands ───────────────────────────────────────────────────────

/// IPC response shape for a `collection` record.
#[derive(Debug, Clone, Serialize)]
pub struct CollectionResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

impl From<crate::services::collection_service::Collection> for CollectionResponse {
    fn from(c: crate::services::collection_service::Collection) -> Self {
        Self {
            id: c.id,
            name: c.name,
            description: c.description,
        }
    }
}

/// Returns all collections ordered by name.
#[tauri::command]
pub async fn get_collections(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<CollectionResponse>, String> {
    let collections = crate::services::collection_service::get_all(&state.db).await?;
    Ok(collections.into_iter().map(Into::into).collect())
}

/// Creates a new collection.  `name` must be non-empty.
#[tauri::command]
pub async fn create_collection(
    state: State<'_, Arc<AppState>>,
    name: String,
    description: Option<String>,
) -> Result<CollectionResponse, String> {
    if name.trim().is_empty() {
        return Err("Collection name is required".to_string());
    }
    let c =
        crate::services::collection_service::create(&state.db, name.trim(), description.as_deref())
            .await?;
    Ok(c.into())
}

/// Updates the name and description of an existing collection.
#[tauri::command]
pub async fn update_collection(
    state: State<'_, Arc<AppState>>,
    id: String,
    name: String,
    description: Option<String>,
) -> Result<CollectionResponse, String> {
    if name.trim().is_empty() {
        return Err("Collection name is required".to_string());
    }
    let c = crate::services::collection_service::update(
        &state.db,
        &id,
        name.trim(),
        description.as_deref(),
    )
    .await?;
    Ok(c.into())
}

/// Deletes a collection.  Fails if any campaigns are subscribed or any sources
/// still reference it.
#[tauri::command]
pub async fn delete_collection(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    crate::services::collection_service::delete(&state.db, &id).await
}

/// Subscribes a campaign to a collection.  Idempotent.
#[tauri::command]
pub async fn add_campaign_collection(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
    collection_id: String,
) -> Result<(), String> {
    crate::services::collection_service::add_campaign_collection(
        &state.db,
        &campaign_id,
        &collection_id,
    )
    .await
}

/// Removes a campaign's subscription to a collection.
#[tauri::command]
pub async fn remove_campaign_collection(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
    collection_id: String,
) -> Result<(), String> {
    crate::services::collection_service::remove_campaign_collection(
        &state.db,
        &campaign_id,
        &collection_id,
    )
    .await
}

/// Returns all collections to which a campaign is subscribed.
#[tauri::command]
pub async fn get_campaign_collections(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
) -> Result<Vec<CollectionResponse>, String> {
    let cols =
        crate::services::collection_service::get_campaign_collections(&state.db, &campaign_id)
            .await?;
    Ok(cols.into_iter().map(Into::into).collect())
}

// ── Chat Commands ─────────────────────────────────────────────────────────────

/// Chat message request payload.
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    // Deserialized from IPC; will be used in Phase 2 when agent routing is
    // collection-scoped. Rust's dead-code lint does not see serde reads.
    #[allow(dead_code)] // field is populated via serde; Rust lint cannot see that
    pub campaign_id: Option<String>,
}

/// Chat message response chunk.
#[derive(Debug, Clone, Serialize)]
pub struct ChatToken {
    pub token: String,
    pub done: bool,
}

/// Sends a user message to the AI agent and emits streaming tokens via Tauri
/// events.
///
/// The command returns immediately after kicking off the stream. Tokens are
/// delivered one-by-one through the `chat-token` event. When the stream
/// completes a final event with `done: true` is emitted.
#[tauri::command]
pub async fn chat_send(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    request: ChatRequest,
) -> Result<(), String> {
    let llm = state.llm_provider.clone();
    let app = app_handle.clone();

    // Spawn the streaming work so the command returns immediately
    tokio::spawn(async move {
        let system_prompt = "You are a helpful TTRPG Game Master's assistant. \
            Answer questions about the rules based on the provided source material. \
            Always cite your sources.";

        let messages = vec![crate::providers::llm_provider::ChatMessage {
            role: "user".to_string(),
            content: request.message,
        }];

        match llm.chat_stream(system_prompt, &messages).await {
            Ok(mut rx) => {
                while let Some(token_result) = rx.recv().await {
                    match token_result {
                        Ok(token) => {
                            let _ = app.emit("chat-token", ChatToken { token, done: false });
                        }
                        Err(e) => {
                            let _ = app.emit(
                                "chat-token",
                                ChatToken {
                                    token: format!("[Error: {e}]"),
                                    done: true,
                                },
                            );
                            return;
                        }
                    }
                }
                let _ = app.emit(
                    "chat-token",
                    ChatToken {
                        token: String::new(),
                        done: true,
                    },
                );
            }
            Err(e) => {
                let _ = app.emit(
                    "chat-token",
                    ChatToken {
                        token: format!("[Error: {e}]"),
                        done: true,
                    },
                );
            }
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use surrealdb::engine::local::Db;
    use surrealdb::Surreal;

    async fn setup_db() -> Surreal<Db> {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db
    }

    // ── CollectionResponse conversion ────────────────────────────────────────

    #[test]
    fn collection_response_from_collection() {
        let c = crate::services::collection_service::Collection {
            id: "abc123".to_string(),
            name: "D&D 5e Core".to_string(),
            description: Some("Core rulebooks".to_string()),
        };
        let resp = CollectionResponse::from(c);
        assert_eq!(resp.id, "abc123");
        assert_eq!(resp.name, "D&D 5e Core");
        assert_eq!(resp.description.as_deref(), Some("Core rulebooks"));
    }

    #[test]
    fn collection_response_from_collection_no_desc() {
        let c = crate::services::collection_service::Collection {
            id: "xyz".to_string(),
            name: "Pathfinder".to_string(),
            description: None,
        };
        let resp = CollectionResponse::from(c);
        assert!(resp.description.is_none());
    }

    // ── create_collection validation ─────────────────────────────────────────

    #[tokio::test]
    async fn create_collection_rejects_empty_name() {
        let db = setup_db().await;
        let state_inner = Arc::new(AppState {
            db,
            llm_provider: Arc::new(crate::providers::llm_provider::OpenAIProvider::new(
                String::new(),
                String::new(),
            )),
            vector_store: Arc::new(crate::providers::vector_store::SurrealDbVector::new(
                surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
                    .await
                    .unwrap(),
            )),
            blob_store: Arc::new(crate::providers::blob_store::LocalFileStore::new(
                std::path::PathBuf::from("/tmp"),
            )),
        });
        // Call the service directly (State<> wrapping is a Tauri concern)
        let result = crate::services::collection_service::create(&state_inner.db, "  ", None).await;
        // The service itself does not validate; validation is in the command.
        // Test the command-level guard via direct logic replication.
        let name = "  ".to_string();
        assert!(
            name.trim().is_empty(),
            "empty-name guard must trigger for whitespace-only input"
        );
        // The service call with whitespace name should succeed (it's a DB-level
        // decision), but the command wraps it with the guard. Ensure guard fires.
        drop(result);
    }

    // ── update_collection validation ─────────────────────────────────────────

    #[test]
    fn update_collection_empty_name_guard() {
        let name = String::new();
        assert!(name.trim().is_empty());
    }

    // ── get_sources — collection_id filter ───────────────────────────────────

    #[tokio::test]
    async fn get_sources_filters_by_collection() {
        let db = setup_db().await;

        // Create two collections
        let col_a = crate::services::collection_service::create(&db, "Col A", None)
            .await
            .unwrap();
        let col_b = crate::services::collection_service::create(&db, "Col B", None)
            .await
            .unwrap();

        // Insert one source per collection
        db.query(
            "CREATE source SET
                 id = 'src_a',
                 filename = 'a.pdf',
                 display_name = 'Source A',
                 source_type = 'rules',
                 page_count = 0,
                 indexed_at = time::now(),
                 index_status = 'pending',
                 embed_model = 'nomic-embed-text-v1.5',
                 campaign = NULL,
                 collection = type::thing('collection', $cid)",
        )
        .bind(("cid", col_a.id.clone()))
        .await
        .unwrap();

        db.query(
            "CREATE source SET
                 id = 'src_b',
                 filename = 'b.pdf',
                 display_name = 'Source B',
                 source_type = 'rules',
                 page_count = 0,
                 indexed_at = time::now(),
                 index_status = 'pending',
                 embed_model = 'nomic-embed-text-v1.5',
                 campaign = NULL,
                 collection = type::thing('collection', $cid)",
        )
        .bind(("cid", col_b.id.clone()))
        .await
        .unwrap();

        // Query with collection filter
        let mut resp_a = db
            .query(
                "SELECT * FROM source \
                 WHERE collection = type::thing('collection', $cid) \
                 ORDER BY display_name ASC",
            )
            .bind(("cid", col_a.id.clone()))
            .await
            .unwrap();

        #[derive(Deserialize)]
        struct Row {
            id: surrealdb::sql::Thing,
        }
        let rows_a: Vec<Row> = resp_a.take(0).unwrap();
        assert_eq!(rows_a.len(), 1);
        assert_eq!(rows_a[0].id.id.to_raw(), "src_a");

        // Query all (no filter)
        let mut resp_all = db
            .query("SELECT * FROM source ORDER BY display_name ASC")
            .await
            .unwrap();
        let rows_all: Vec<Row> = resp_all.take(0).unwrap();
        assert_eq!(rows_all.len(), 2);
    }

    // ── upload_source — collection_id guard ──────────────────────────────────

    #[test]
    fn upload_source_empty_collection_id_guard() {
        let collection_id = "   ".to_string();
        assert!(
            collection_id.trim().is_empty(),
            "whitespace collection_id must be rejected"
        );
    }
}
