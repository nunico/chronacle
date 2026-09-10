use serde::Deserialize;
use tauri::{AppHandle, State};

use crate::exit_guard::{ExitAuthorization, ExitResolution};

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
pub(crate) fn cancel_app_exit(
    _app: AppHandle,
    intent: u64,
    authorization: State<'_, ExitAuthorization>,
) -> Result<bool, String> {
    match authorization.begin_resolution(intent, false) {
        ExitResolution::Stale => Ok(false),
        ExitResolution::Cancelled => Ok(true),
        ExitResolution::TauriExit => Err("Invalid close cancellation state.".to_string()),
        #[cfg(any(test, all(target_os = "macos", feature = "rocksdb")))]
        ExitResolution::NativeReply { terminate } => {
            schedule_native_reply(&_app, &authorization, intent, terminate)?;
            Ok(true)
        }
    }
}

#[tauri::command]
pub(crate) fn confirm_app_exit(
    app: AppHandle,
    intent: u64,
    _decision: ExitDecision,
    authorization: State<'_, ExitAuthorization>,
) -> Result<(), String> {
    match authorization.begin_resolution(intent, true) {
        ExitResolution::Stale | ExitResolution::Cancelled => {
            Err("The close request is no longer active.".to_string())
        }
        ExitResolution::TauriExit => {
            app.exit(0);
            Ok(())
        }
        #[cfg(any(test, all(target_os = "macos", feature = "rocksdb")))]
        ExitResolution::NativeReply { terminate } => {
            schedule_native_reply(&app, &authorization, intent, terminate)
        }
    }
}

#[cfg(any(test, all(target_os = "macos", feature = "rocksdb")))]
fn schedule_native_reply(
    app: &AppHandle,
    authorization: &ExitAuthorization,
    intent: u64,
    terminate: bool,
) -> Result<(), String> {
    #[cfg(all(target_os = "macos", feature = "rocksdb"))]
    {
        return crate::macos_exit::schedule_reply(app, intent, terminate).map_err(|error| {
            authorization.rollback_native_resolution(intent, terminate);
            error
        });
    }

    #[cfg(all(test, not(all(target_os = "macos", feature = "rocksdb"))))]
    {
        authorization.rollback_native_resolution(intent, terminate);
        let _ = app;
        Err("A native close reply was requested on an unsupported platform.".to_string())
    }
}
