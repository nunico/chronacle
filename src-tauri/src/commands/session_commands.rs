use std::sync::Arc;

use tauri::State;

use crate::services::entity_service::GraphNode;
use crate::services::session_service::{self, Session, SessionError, SessionInput};
use crate::AppState;

#[tauri::command]
pub async fn create_session(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
    input: SessionInput,
) -> Result<Session, SessionError> {
    session_service::create(&state.db, &campaign_id, input).await
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
    campaign_id: String,
    input: SessionInput,
) -> Result<Session, SessionError> {
    // campaign_id is accepted for IPC compatibility but not forwarded to the
    // service — the session record already carries its campaign FK.
    let _ = campaign_id;
    session_service::update(&state.db, &id, input).await
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
        let _ = update_session as fn(_, _, _, _) -> _;
        let _ = delete_session as fn(_, _) -> _;
        let _ = get_session_entities as fn(_, _) -> _;
    }
}
