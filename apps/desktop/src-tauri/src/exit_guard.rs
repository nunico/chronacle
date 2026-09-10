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

#[derive(Default)]
struct AuthorizationState {
    next_intent: u64,
    pending_intent: Option<u64>,
    authorized: bool,
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
        intent
    }

    pub(crate) fn revoke(&self, intent: u64) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.pending_intent != Some(intent) || state.authorized {
            return false;
        }
        state.pending_intent = None;
        true
    }

    pub(crate) fn authorize(&self, intent: u64) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.pending_intent != Some(intent) || state.authorized {
            return false;
        }
        state.authorized = true;
        true
    }

    #[cfg_attr(not(feature = "rocksdb"), allow(dead_code))]
    pub(crate) fn intercept_exit(&self) -> ExitRequestDecision {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.authorized {
            state.authorized = false;
            state.pending_intent = None;
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
}

#[cfg(test)]
mod tests {
    use super::{ExitAuthorization, ExitRequestDecision};

    #[test]
    fn concurrent_requests_share_one_pending_intent() {
        let authorization = ExitAuthorization::default();

        let first = authorization.request();
        let second = authorization.request();

        assert_eq!(first, second);
        assert_ne!(first, 0);
    }

    #[test]
    fn only_the_pending_intent_can_authorize_one_exit() {
        let authorization = ExitAuthorization::default();
        let intent = authorization.request();

        assert!(!authorization.authorize(intent + 1));
        assert!(authorization.authorize(intent));
        assert!(!authorization.authorize(intent));
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

        assert!(!authorization.revoke(intent + 1));
        assert_eq!(authorization.request(), intent);
        assert!(authorization.revoke(intent));
        assert!(!authorization.authorize(intent));
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
}
