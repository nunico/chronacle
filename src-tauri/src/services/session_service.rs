use std::sync::Arc;

use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use thiserror::Error;

use crate::providers::embedding::EmbeddingProvider;

// ── Error type ──────────────────────────────────────────────────────────────

#[derive(Debug, Error, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "code")]
pub enum SessionError {
    #[error("Session '{id}' not found")]
    NotFound { id: String },
    #[error("Validation error on field '{field}': {message}")]
    Validation { field: String, message: String },
    #[error("Database error: {message}")]
    Database { message: String },
}

// ── Internal record ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
struct SessionRecord {
    pub id: Thing,
    pub campaign: Option<Thing>,
    pub session_number: i64,
    pub title: String,
    pub date_played: String,
    pub notes: String,
    #[serde(default)]
    pub is_gm_only: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl From<SessionRecord> for Session {
    fn from(r: SessionRecord) -> Self {
        Self {
            id: r.id.id.to_raw(),
            campaign_id: r.campaign.map(|t| t.id.to_raw()),
            session_number: r.session_number,
            title: r.title,
            date_played: r.date_played,
            notes: r.notes,
            is_gm_only: r.is_gm_only,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

// ── Public DTOs ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub id: String,
    pub campaign_id: Option<String>,
    pub session_number: i64,
    pub title: String,
    pub date_played: String,
    pub notes: String,
    pub is_gm_only: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInput {
    pub session_number: i64,
    pub title: String,
    pub date_played: String,
    pub notes: String,
    #[serde(default)]
    pub is_gm_only: Option<bool>,
}

// ── Service functions ────────────────────────────────────────────────────────

/// Create a new session scoped to the given campaign.
pub async fn create<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
    input: SessionInput,
) -> Result<Session, SessionError> {
    if input.title.trim().is_empty() {
        return Err(SessionError::Validation {
            field: "title".to_string(),
            message: "Title is required".to_string(),
        });
    }
    if input.session_number <= 0 {
        return Err(SessionError::Validation {
            field: "session_number".to_string(),
            message: "Session number must be greater than 0".to_string(),
        });
    }

    let id = uuid::Uuid::new_v4().to_string().replace('-', "");
    let mut response = db
        .query(
            "CREATE type::thing('session', $id) SET
                campaign       = type::thing('campaign', $campaign_id),
                session_number = $session_number,
                title          = $title,
                date_played    = $date_played,
                notes          = $notes,
                is_gm_only      = $gm_only,
                created_at     = time::now(),
                updated_at     = time::now()",
        )
        .bind(("id", id.clone()))
        .bind(("campaign_id", campaign_id.to_owned()))
        .bind(("session_number", input.session_number))
        .bind(("title", input.title.trim().to_owned()))
        .bind(("date_played", input.date_played.clone()))
        .bind(("notes", input.notes.clone()))
        .bind(("gm_only", input.is_gm_only.unwrap_or(false)))
        .await
        .map_err(|e| SessionError::Database {
            message: e.to_string(),
        })?;

    let records: Vec<SessionRecord> = response.take(0).map_err(|e| SessionError::Database {
        message: e.to_string(),
    })?;

    let session =
        records
            .into_iter()
            .next()
            .map(Into::into)
            .ok_or_else(|| SessionError::Database {
                message: "No record returned after create".to_string(),
            })?;

    // Resolve wikilinks — failure must not block the save.
    // Awaited synchronously: wikilink resolution is fast in practice (entity
    // count is small), and spawning would require C: Clone + Send + 'static
    // which conflicts with the generic bound used in tests.
    let _ = crate::services::wikilink::parse_and_sync_wikilinks(
        db,
        "session",
        &id,
        &input.notes,
        crate::services::wikilink::WikilinkScope::Campaign { campaign_id },
    )
    .await;

    Ok(session)
}

/// List all sessions for a campaign, ordered by session_number ascending.
pub async fn get_all<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
) -> Result<Vec<Session>, SessionError> {
    let mut response = db
        .query(
            "SELECT * FROM session \
             WHERE campaign = type::thing('campaign', $campaign_id) \
             ORDER BY session_number ASC",
        )
        .bind(("campaign_id", campaign_id.to_owned()))
        .await
        .map_err(|e| SessionError::Database {
            message: e.to_string(),
        })?;

    let records: Vec<SessionRecord> = response.take(0).map_err(|e| SessionError::Database {
        message: e.to_string(),
    })?;

    Ok(records.into_iter().map(Into::into).collect())
}

/// Fetch a single session by its raw ID.
pub async fn get_by_id<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    id: &str,
) -> Result<Session, SessionError> {
    let mut response = db
        .query("SELECT * FROM type::thing('session', $id)")
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| SessionError::Database {
            message: e.to_string(),
        })?;

    let records: Vec<SessionRecord> = response.take(0).map_err(|e| SessionError::Database {
        message: e.to_string(),
    })?;

    records
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| SessionError::NotFound { id: id.to_string() })
}

/// Update an existing session. Returns `NotFound` if the record doesn't exist.
pub async fn update<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    id: &str,
    input: SessionInput,
) -> Result<Session, SessionError> {
    if input.title.trim().is_empty() {
        return Err(SessionError::Validation {
            field: "title".to_string(),
            message: "Title is required".to_string(),
        });
    }
    if input.session_number <= 0 {
        return Err(SessionError::Validation {
            field: "session_number".to_string(),
            message: "Session number must be positive".to_string(),
        });
    }

    let mut response = db
        .query(
            "UPDATE type::thing('session', $id) SET
                session_number = $session_number,
                title          = $title,
                date_played    = $date_played,
                notes          = $notes,
                is_gm_only      = $gm_only,
                updated_at     = time::now()",
        )
        .bind(("id", id.to_owned()))
        .bind(("session_number", input.session_number))
        .bind(("title", input.title.trim().to_owned()))
        .bind(("date_played", input.date_played.clone()))
        .bind(("notes", input.notes.clone()))
        .bind(("gm_only", input.is_gm_only.unwrap_or(false)))
        .await
        .map_err(|e| SessionError::Database {
            message: e.to_string(),
        })?;

    let records: Vec<SessionRecord> = response.take(0).map_err(|e| SessionError::Database {
        message: e.to_string(),
    })?;

    let record = records
        .into_iter()
        .next()
        .ok_or_else(|| SessionError::NotFound { id: id.to_string() })?;

    // Extract campaign_id from the record before converting it into a Session.
    let campaign_id_for_wikilinks = record
        .campaign
        .as_ref()
        .map(|t| t.id.to_raw())
        .unwrap_or_default();

    let session: Session = record.into();

    // Resolve wikilinks — failure must not block the save.
    // Awaited synchronously: wikilink resolution is fast in practice (entity
    // count is small), and spawning would require C: Clone + Send + 'static
    // which conflicts with the generic bound used in tests.
    let _ = crate::services::wikilink::parse_and_sync_wikilinks(
        db,
        "session",
        id,
        &input.notes,
        crate::services::wikilink::WikilinkScope::Campaign {
            campaign_id: &campaign_id_for_wikilinks,
        },
    )
    .await;

    Ok(session)
}

/// Embed a session's text (title + notes) and persist the vector + model ID
/// onto the record, so session notes participate in semantic retrieval.
///
/// A zero-length vector (e.g. a mock provider whose model isn't ready) is a
/// no-op rather than an error — embedding must never block a session save.
pub async fn embed_session<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    embed: &Arc<dyn EmbeddingProvider>,
    session: &Session,
) -> Result<(), SessionError> {
    let mut text = session.title.trim().to_owned();
    let notes = session.notes.trim();
    if !notes.is_empty() {
        text.push('\n');
        text.push_str(notes);
    }
    let vecs = embed
        .embed_documents(vec![text])
        .await
        .map_err(|e| SessionError::Database {
            message: e.to_string(),
        })?;
    let vec = vecs.into_iter().next().unwrap_or_default();
    if vec.is_empty() {
        return Ok(());
    }
    let model = embed.model_name().to_owned();
    db.query("UPDATE type::thing('session', $id) SET embedding = $vec, embed_model = $model")
        .bind(("id", session.id.clone()))
        .bind(("vec", vec))
        .bind(("model", model))
        .await
        .map_err(|e| SessionError::Database {
            message: e.to_string(),
        })?;
    Ok(())
}

/// Hard-delete a session by its raw ID.
pub async fn delete<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    id: &str,
) -> Result<(), SessionError> {
    db.query("DELETE type::thing('session', $id)")
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| SessionError::Database {
            message: e.to_string(),
        })?;
    Ok(())
}

/// Return the entities (events) linked to this session.
///
/// Queries events that reference the session directly.  In Phase 3 when
/// explicit session→entity edges are added to the schema, this can be enhanced.
pub async fn get_entities<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    session_id: &str,
) -> Result<Vec<crate::services::entity_service::GraphNode>, SessionError> {
    crate::services::entity_service::get_events_for_session(db, session_id)
        .await
        .map_err(|e| SessionError::Database {
            message: e.to_string(),
        })
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that `Session` fields serialise to the expected JSON keys,
    /// which is what the Tauri IPC bridge sends to the frontend.
    #[test]
    fn session_dto_serialises_correctly() {
        let session = Session {
            id: "abc123".to_string(),
            campaign_id: Some("camp1".to_string()),
            session_number: 3,
            title: "The Heist".to_string(),
            date_played: "2026-06-05".to_string(),
            notes: "Notes here".to_string(),
            is_gm_only: false,
            created_at: None,
            updated_at: None,
        };
        let v = serde_json::to_value(&session).unwrap();
        assert_eq!(v["id"], "abc123");
        assert_eq!(v["campaign_id"], "camp1");
        assert_eq!(v["session_number"], 3);
        assert_eq!(v["title"], "The Heist");
        assert_eq!(v["date_played"], "2026-06-05");
    }

    #[tokio::test]
    async fn create_and_update_persist_is_gm_only() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='T', system='5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        let created = create(
            &db,
            "camp1",
            SessionInput {
                session_number: 1,
                title: "Secret Session".to_string(),
                date_played: "2026-06-05".to_string(),
                notes: "GM-only recap".to_string(),
                is_gm_only: Some(true),
            },
        )
        .await
        .unwrap();
        assert!(created.is_gm_only);
        assert!(get_by_id(&db, &created.id).await.unwrap().is_gm_only);

        let toggled = update(
            &db,
            &created.id,
            SessionInput {
                session_number: 1,
                title: "Secret Session".to_string(),
                date_played: "2026-06-05".to_string(),
                notes: "GM-only recap".to_string(),
                is_gm_only: Some(false),
            },
        )
        .await
        .unwrap();
        assert!(!toggled.is_gm_only);
    }

    #[tokio::test]
    async fn embed_session_populates_embedding_and_model() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        let session = create(
            &db,
            "camp1",
            SessionInput {
                session_number: 1,
                title: "The Awakening".to_string(),
                date_played: "2026-06-05".to_string(),
                notes: "The party met in the tavern and took the job.".to_string(),
                is_gm_only: None,
            },
        )
        .await
        .unwrap();

        let embed: Arc<dyn EmbeddingProvider> =
            Arc::new(crate::providers::embedding::MockEmbeddingProvider::new(768));
        embed_session(&db, &embed, &session).await.unwrap();

        #[derive(Deserialize)]
        struct Row {
            embedding: Option<Vec<f32>>,
            embed_model: Option<String>,
        }
        let mut resp = db
            .query("SELECT embedding, embed_model FROM type::thing('session', $id)")
            .bind(("id", session.id.clone()))
            .await
            .unwrap();
        let rows: Vec<Row> = resp.take(0).unwrap();
        let row = rows.into_iter().next().expect("session row");
        assert_eq!(row.embedding.as_ref().map(|v| v.len()), Some(768));
        assert_eq!(row.embed_model.as_deref(), Some("mock"));
    }
}
