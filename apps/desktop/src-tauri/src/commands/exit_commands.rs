use serde::Deserialize;
use tauri::{AppHandle, State};

use crate::exit_guard::ExitAuthorization;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExitDecision {
    Keep,
    Discard,
}

#[tauri::command]
pub(crate) fn request_app_exit(authorization: State<'_, ExitAuthorization>) -> u64 {
    authorization.request()
}

#[tauri::command]
pub(crate) fn pending_app_exit(authorization: State<'_, ExitAuthorization>) -> Option<u64> {
    authorization.pending()
}

#[tauri::command]
pub(crate) fn cancel_app_exit(intent: u64, authorization: State<'_, ExitAuthorization>) -> bool {
    authorization.revoke(intent)
}

#[tauri::command]
pub(crate) fn confirm_app_exit(
    app: AppHandle,
    intent: u64,
    _decision: ExitDecision,
    authorization: State<'_, ExitAuthorization>,
) -> Result<(), String> {
    if !authorization.authorize(intent) {
        return Err("The close request is no longer active.".to_string());
    }
    app.exit(0);
    Ok(())
}
