use std::sync::Mutex;

#[cfg(feature = "rocksdb")]
use serde::Serialize;

#[cfg(feature = "rocksdb")]
#[derive(Clone, Copy, Debug, Serialize)]
pub(crate) struct ExitRequestPayload {
    pub(crate) intent: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(feature = "rocksdb"), allow(dead_code))]
pub(crate) enum ExitRequestDecision {
    Authorized,
    Prevent { intent: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NativeExitRequest {
    intent: u64,
    should_emit: bool,
    authorized: bool,
}

impl NativeExitRequest {
    pub(crate) fn intent(self) -> u64 {
        self.intent
    }

    pub(crate) fn should_emit(self) -> bool {
        self.should_emit
    }

    pub(crate) fn is_authorized(self) -> bool {
        self.authorized
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExitResolution {
    Stale,
    Cancelled,
    TauriExit,
    NativeReply { terminate: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeReplyState {
    Deferred,
    Scheduled { terminate: bool },
}

#[derive(Default)]
struct AuthorizationState {
    next_intent: u64,
    pending_intent: Option<u64>,
    authorized: bool,
    native_reply: Option<NativeReplyState>,
    native_created_intent: bool,
}

#[derive(Default)]
pub(crate) struct ExitAuthorization {
    state: Mutex<AuthorizationState>,
}

impl ExitAuthorization {
    pub(crate) fn request(&self) -> u64 {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(intent) = state.pending_intent {
            return intent;
        }
        state.next_intent = state
            .next_intent
            .checked_add(1)
            .expect("exit intent sequence exhausted");
        let intent = state.next_intent;
        state.pending_intent = Some(intent);
        state.native_created_intent = false;
        intent
    }

    pub(crate) fn pending(&self) -> Option<u64> {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.pending_intent
    }

    #[cfg_attr(not(feature = "rocksdb"), allow(dead_code))]
    pub(crate) fn intercept_exit(&self) -> ExitRequestDecision {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.authorized {
            state.authorized = false;
            state.pending_intent = None;
            state.native_reply = None;
            state.native_created_intent = false;
            return ExitRequestDecision::Authorized;
        }
        let intent = if let Some(intent) = state.pending_intent {
            intent
        } else {
            state.next_intent = state
                .next_intent
                .checked_add(1)
                .expect("exit intent sequence exhausted");
            let intent = state.next_intent;
            state.pending_intent = Some(intent);
            intent
        };
        ExitRequestDecision::Prevent { intent }
    }

    pub(crate) fn intercept_native_exit(&self) -> NativeExitRequest {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.authorized {
            let intent = state.pending_intent.unwrap_or_default();
            state.authorized = false;
            state.pending_intent = None;
            state.native_reply = None;
            state.native_created_intent = false;
            return NativeExitRequest {
                intent,
                should_emit: false,
                authorized: true,
            };
        }
        let should_emit = state.pending_intent.is_none();
        let intent = if let Some(intent) = state.pending_intent {
            intent
        } else {
            state.next_intent = state
                .next_intent
                .checked_add(1)
                .expect("exit intent sequence exhausted");
            let intent = state.next_intent;
            state.pending_intent = Some(intent);
            state.native_created_intent = true;
            intent
        };
        state.native_reply.get_or_insert(NativeReplyState::Deferred);
        NativeExitRequest {
            intent,
            should_emit,
            authorized: false,
        }
    }

    pub(crate) fn begin_resolution(&self, intent: u64, terminate: bool) -> ExitResolution {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.pending_intent != Some(intent) || state.authorized {
            return ExitResolution::Stale;
        }
        match state.native_reply {
            Some(NativeReplyState::Deferred) => {
                state.native_reply = Some(NativeReplyState::Scheduled { terminate });
                ExitResolution::NativeReply { terminate }
            }
            Some(NativeReplyState::Scheduled { .. }) => ExitResolution::Stale,
            None if terminate => {
                state.authorized = true;
                ExitResolution::TauriExit
            }
            None => {
                state.pending_intent = None;
                state.native_created_intent = false;
                ExitResolution::Cancelled
            }
        }
    }

    pub(crate) fn rollback_native_resolution(&self, intent: u64, terminate: bool) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.pending_intent != Some(intent)
            || state.native_reply != Some(NativeReplyState::Scheduled { terminate })
        {
            return false;
        }
        state.native_reply = Some(NativeReplyState::Deferred);
        true
    }

    pub(crate) fn complete_native_reply(&self, intent: u64, terminate: bool) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.pending_intent != Some(intent)
            || state.native_reply != Some(NativeReplyState::Scheduled { terminate })
        {
            return false;
        }
        state.pending_intent = None;
        state.native_reply = None;
        state.native_created_intent = false;
        true
    }

    pub(crate) fn rollback_native_request(&self, intent: u64) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.pending_intent != Some(intent)
            || !state.native_created_intent
            || state.native_reply != Some(NativeReplyState::Deferred)
        {
            return false;
        }
        state.pending_intent = None;
        state.native_reply = None;
        state.native_created_intent = false;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{ExitAuthorization, ExitRequestDecision, ExitResolution};

    #[test]
    fn concurrent_requests_share_one_pending_intent() {
        let authorization = ExitAuthorization::default();

        let first = authorization.request();
        let second = authorization.request();

        assert_eq!(first, second);
        assert_ne!(first, 0);
        assert_eq!(authorization.pending(), Some(first));
    }

    #[test]
    fn only_the_pending_intent_can_authorize_one_exit() {
        let authorization = ExitAuthorization::default();
        let intent = authorization.request();

        assert_eq!(
            authorization.begin_resolution(intent + 1, true),
            ExitResolution::Stale
        );
        assert_eq!(
            authorization.begin_resolution(intent, true),
            ExitResolution::TauriExit
        );
        assert_eq!(
            authorization.begin_resolution(intent, true),
            ExitResolution::Stale
        );
        assert_eq!(
            authorization.intercept_exit(),
            ExitRequestDecision::Authorized
        );

        let next = authorization.intercept_exit();
        assert!(matches!(next, ExitRequestDecision::Prevent { intent: next } if next > intent));
    }

    #[test]
    fn revocation_is_scoped_and_a_later_request_has_a_new_identity() {
        let authorization = ExitAuthorization::default();
        let intent = authorization.request();

        assert_eq!(
            authorization.begin_resolution(intent + 1, false),
            ExitResolution::Stale
        );
        assert_eq!(authorization.request(), intent);
        assert_eq!(
            authorization.begin_resolution(intent, false),
            ExitResolution::Cancelled
        );
        assert_eq!(authorization.pending(), None);
        assert_eq!(
            authorization.begin_resolution(intent, true),
            ExitResolution::Stale
        );
        assert!(authorization.request() > intent);
    }

    #[test]
    fn unauthorized_native_exit_is_prevented_and_publishes_an_intent() {
        let authorization = ExitAuthorization::default();

        let first = authorization.intercept_exit();
        let second = authorization.intercept_exit();

        assert!(matches!(first, ExitRequestDecision::Prevent { intent } if intent > 0));
        assert_eq!(second, first);
    }

    #[test]
    fn default_capability_has_no_frontend_exit_permission() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json"))
                .expect("default capability is valid JSON");
        let permissions = capability["permissions"]
            .as_array()
            .expect("permissions is an array");

        for permission in permissions {
            let permission = permission.as_str().expect("permission is a string");
            assert_ne!(permission, "core:window:allow-close");
            assert_ne!(permission, "core:window:allow-destroy");
            assert!(
                !permission.contains('*'),
                "wildcard permission: {permission}"
            );
        }
    }

    #[test]
    fn cancelling_a_native_transaction_allows_a_later_native_quit() {
        let authorization = ExitAuthorization::default();
        let first = authorization.intercept_native_exit();
        let first_intent = first.intent();

        assert_eq!(
            authorization.begin_resolution(first_intent, false),
            ExitResolution::NativeReply { terminate: false }
        );
        assert!(authorization.complete_native_reply(first_intent, false));

        let second = authorization.intercept_native_exit();
        assert!(second.intent() > first_intent);
        assert!(second.should_emit());
    }

    #[test]
    fn native_quit_attaches_to_an_existing_frontend_intent() {
        let authorization = ExitAuthorization::default();
        let intent = authorization.request();

        let native = authorization.intercept_native_exit();

        assert_eq!(native.intent(), intent);
        assert!(!native.should_emit());
        assert_eq!(
            authorization.begin_resolution(intent, true),
            ExitResolution::NativeReply { terminate: true }
        );
    }

    #[test]
    fn native_confirmation_is_reserved_once_and_does_not_authorize_tauri_exit() {
        let authorization = ExitAuthorization::default();
        let native = authorization.intercept_native_exit();
        let intent = native.intent();

        assert_eq!(
            authorization.begin_resolution(intent, true),
            ExitResolution::NativeReply { terminate: true }
        );
        assert_eq!(
            authorization.begin_resolution(intent, true),
            ExitResolution::Stale
        );
        assert!(authorization.complete_native_reply(intent, true));
        assert!(matches!(
            authorization.intercept_exit(),
            ExitRequestDecision::Prevent { .. }
        ));
    }

    #[test]
    fn authorized_tauri_exit_passes_through_a_racing_native_termination() {
        let authorization = ExitAuthorization::default();
        let intent = authorization.request();
        assert_eq!(
            authorization.begin_resolution(intent, true),
            ExitResolution::TauriExit
        );

        let native = authorization.intercept_native_exit();

        assert!(native.is_authorized());
        assert_eq!(authorization.pending(), None);
        assert!(matches!(
            authorization.intercept_exit(),
            ExitRequestDecision::Prevent { intent: next } if next > intent
        ));
    }

    #[test]
    fn failed_native_reply_scheduling_restores_retryable_pending_state() {
        let authorization = ExitAuthorization::default();
        let native = authorization.intercept_native_exit();
        let intent = native.intent();
        assert_eq!(
            authorization.begin_resolution(intent, false),
            ExitResolution::NativeReply { terminate: false }
        );

        assert!(authorization.rollback_native_resolution(intent, false));
        assert_eq!(authorization.pending(), Some(intent));
        assert_eq!(
            authorization.begin_resolution(intent, false),
            ExitResolution::NativeReply { terminate: false }
        );
    }

    #[test]
    fn failed_native_event_emission_rolls_back_only_its_new_intent() {
        let authorization = ExitAuthorization::default();
        let native = authorization.intercept_native_exit();
        let intent = native.intent();

        assert!(native.should_emit());
        assert!(authorization.rollback_native_request(intent));
        assert_eq!(authorization.pending(), None);

        let frontend_intent = authorization.request();
        let attached = authorization.intercept_native_exit();
        assert_eq!(attached.intent(), frontend_intent);
        assert!(!attached.should_emit());
        assert!(!authorization.rollback_native_request(frontend_intent));
        assert_eq!(authorization.pending(), Some(frontend_intent));
    }
}
