use std::sync::Arc;

use tauri::State;

use crate::AppState;
use chronacle_domain::session_service::{self, Session, SessionError, SessionInput};
use chronacle_extraction::entity_service::GraphNode;

/// Embed a session's notes for semantic retrieval after a create/update.
///
/// Embedding failure is logged but never fails the save — the session is still
/// persisted; it just won't surface in semantic search until the next edit.
async fn embed_after_save(state: &AppState, session: &Session) {
    let provider = match state.embedding_provider.read() {
        Ok(guard) => guard.clone(),
        Err(e) => {
            eprintln!("session embed: provider lock poisoned: {e}");
            return;
        }
    };
    if let Err(e) = session_service::embed_session(&state.db, &provider, session).await {
        eprintln!(
            "session embed: failed to embed session {}; it will be missing from semantic search: {e}",
            session.id
        );
    }
}

/// Fire the C1 session-notes distillation in the background — best-effort:
/// the save must never fail or block on the LLM.
fn distill_after_save(state: &Arc<AppState>, session: &Session) {
    if session.notes.trim().is_empty() {
        return;
    }
    let llm = match state.llm_provider.read() {
        Ok(guard) => guard.clone(),
        Err(e) => {
            eprintln!("session distill: provider lock poisoned: {e}");
            return;
        }
    };
    let db = state.db.clone();
    let session_id = session.id.clone();
    tokio::spawn(async move {
        match chronacle_extraction::codex_service::distill_session_notes(&db, &llm, &session_id)
            .await
        {
            Ok(n) if n > 0 => eprintln!("session distill: {n} proposal(s) created"),
            Ok(_) => {}
            Err(e) => eprintln!("session distill failed for {session_id}: {e}"),
        }
    });
}

#[tauri::command]
pub async fn create_session(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
    input: SessionInput,
) -> Result<Session, SessionError> {
    let outbound = state.outbound.read().await.clone();
    let session =
        session_service::create(&state.db, &campaign_id, input, outbound.as_ref()).await?;
    embed_after_save(&state, &session).await;
    distill_after_save(state.inner(), &session);
    Ok(session)
}

#[tauri::command]
pub async fn get_sessions(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
) -> Result<Vec<Session>, SessionError> {
    session_service::get_all(&state.db, &campaign_id).await
}

#[tauri::command]
pub async fn get_session(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<Session, SessionError> {
    session_service::get_by_id(&state.db, &id).await
}

#[tauri::command]
pub async fn update_session(
    state: State<'_, Arc<AppState>>,
    id: String,
    input: SessionInput,
) -> Result<Session, SessionError> {
    let outbound = state.outbound.read().await.clone();
    let session = session_service::update(&state.db, &id, input, outbound.as_ref()).await?;
    embed_after_save(&state, &session).await;
    distill_after_save(state.inner(), &session);
    Ok(session)
}

#[tauri::command]
pub async fn delete_session(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), SessionError> {
    session_service::delete(&state.db, &id).await
}

#[tauri::command]
pub async fn get_session_entities(
    state: State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<Vec<GraphNode>, SessionError> {
    session_service::get_entities(&state.db, &session_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: all command functions are referenced so the compiler verifies
    /// that their signatures, imports, and return types are correct.
    #[test]
    fn session_commands_module_compiles() {
        // Taking the address of each async fn forces the compiler to resolve
        // every type used in the signatures without requiring a real Tauri State.
        let _ = create_session as fn(_, _, _) -> _;
        let _ = get_sessions as fn(_, _) -> _;
        let _ = get_session as fn(_, _) -> _;
        let _ = update_session as fn(_, _, _) -> _;
        let _ = delete_session as fn(_, _) -> _;
        let _ = get_session_entities as fn(_, _) -> _;
    }
}
