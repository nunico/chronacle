use crate::exit_guard::{ExitAuthorization, ExitRequestDecision};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeTerminationReply {
    Now,
    Later,
}

fn intercept_native_exit(
    authorization: &ExitAuthorization,
    publish: impl FnOnce(u64),
) -> NativeTerminationReply {
    match authorization.intercept_exit() {
        ExitRequestDecision::Authorized => NativeTerminationReply::Now,
        ExitRequestDecision::Prevent { intent } => {
            publish(intent);
            NativeTerminationReply::Later
        }
    }
}

#[cfg(all(target_os = "macos", feature = "rocksdb"))]
mod platform {
    use std::sync::OnceLock;

    use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, Sel};
    use objc2::sel;
    use objc2_app_kit::{NSApplication, NSApplicationTerminateReply};
    use tauri::{Emitter, Manager};

    use super::{intercept_native_exit, NativeTerminationReply};
    use crate::exit_guard::{ExitAuthorization, ExitRequestPayload};

    static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

    extern "C-unwind" fn application_should_terminate(
        _delegate: &AnyObject,
        _selector: Sel,
        _sender: &NSApplication,
    ) -> NSApplicationTerminateReply {
        let reply = APP_HANDLE
            .get()
            .map(|app| {
                let authorization = app.state::<ExitAuthorization>();
                intercept_native_exit(&authorization, |intent| {
                    if let Err(error) =
                        app.emit_to("main", "app-exit-requested", ExitRequestPayload { intent })
                    {
                        eprintln!("failed to publish macOS application exit request: {error}");
                    }
                })
            })
            .unwrap_or(NativeTerminationReply::Later);

        match reply {
            NativeTerminationReply::Now => NSApplicationTerminateReply::TerminateNow,
            NativeTerminationReply::Later => NSApplicationTerminateReply::TerminateLater,
        }
    }

    pub(super) fn install(app: &tauri::AppHandle) -> Result<(), String> {
        let application =
            NSApplication::sharedApplication(objc2::MainThreadMarker::new().ok_or_else(|| {
                "macOS exit guard must be installed on the main thread".to_string()
            })?);
        let delegate = application.delegate().ok_or_else(|| {
            "macOS application delegate is unavailable; refusing unguarded startup".to_string()
        })?;
        let delegate: &AnyObject = delegate.as_ref();
        let original_class = delegate.class();
        let guarded_class = register_guarded_subclass(original_class)?;

        APP_HANDLE
            .set(app.clone())
            .map_err(|_| "macOS exit guard was installed more than once".to_string())?;

        // SAFETY: `guarded_class` is a direct, zero-ivar subclass of the
        // delegate's current class and overrides one method with the verified
        // AppKit signature. Setup runs on the application main thread before
        // the event loop begins dispatching delegate messages.
        let previous_class = unsafe { AnyObject::set_class(delegate, guarded_class) };
        if previous_class != original_class {
            // SAFETY: restore the class returned by `object_setClass`; it is
            // necessarily compatible with this existing object.
            unsafe { AnyObject::set_class(delegate, previous_class) };
            return Err("macOS application delegate changed during guard installation".to_string());
        }
        Ok(())
    }

    fn register_guarded_subclass(
        original_class: &'static AnyClass,
    ) -> Result<&'static AnyClass, String> {
        let mut builder = ClassBuilder::new(c"ChronacleExitGuardDelegate", original_class)
            .ok_or_else(|| "macOS exit guard class is already registered".to_string())?;

        // SAFETY: this matches AppKit's `-applicationShouldTerminate:` ABI:
        // receiver, selector, NSApplication argument, and
        // NSApplicationTerminateReply return value. The superclass method's
        // encoding is checked by objc2 in debug builds.
        unsafe {
            builder.add_method(
                sel!(applicationShouldTerminate:),
                application_should_terminate
                    as extern "C-unwind" fn(_, _, _) -> NSApplicationTerminateReply,
            );
        }
        Ok(builder.register())
    }
}

#[cfg(all(target_os = "macos", feature = "rocksdb"))]
pub(crate) use platform::install;

#[cfg(test)]
mod tests {
    use super::{intercept_native_exit, NativeTerminationReply};
    use crate::exit_guard::ExitAuthorization;

    #[test]
    fn unauthorized_native_quit_is_deferred_and_publishes_the_shared_intent() {
        let authorization = ExitAuthorization::default();
        let mut published = Vec::new();

        let reply = intercept_native_exit(&authorization, |intent| published.push(intent));

        assert_eq!(reply, NativeTerminationReply::Later);
        assert_eq!(published, vec![authorization.pending().unwrap()]);
    }

    #[test]
    fn repeated_native_quit_requests_coalesce() {
        let authorization = ExitAuthorization::default();
        let mut published = Vec::new();

        assert_eq!(
            intercept_native_exit(&authorization, |intent| published.push(intent)),
            NativeTerminationReply::Later
        );
        assert_eq!(
            intercept_native_exit(&authorization, |intent| published.push(intent)),
            NativeTerminationReply::Later
        );

        assert_eq!(published.len(), 2);
        assert_eq!(published[0], published[1]);
    }

    #[test]
    fn authorized_native_exit_is_not_deferred_or_republished() {
        let authorization = ExitAuthorization::default();
        let intent = authorization.request();
        assert!(authorization.authorize(intent));
        let mut published = Vec::new();

        let reply = intercept_native_exit(&authorization, |intent| published.push(intent));

        assert_eq!(reply, NativeTerminationReply::Now);
        assert!(published.is_empty());
        assert_eq!(authorization.pending(), None);
    }

    #[test]
    fn native_adapter_is_target_scoped_and_preserves_the_tauri_delegate() {
        let manifest = include_str!("../Cargo.toml");
        let target_section = manifest
            .find("[target.'cfg(target_os = \"macos\")'.dependencies]")
            .expect("macOS dependencies are target-scoped");
        let target_dependencies = &manifest[target_section..];
        assert!(target_dependencies.contains("objc2 = \"0.6\""));
        assert!(target_dependencies.contains("objc2-app-kit"));

        let source = include_str!("macos_exit.rs");
        assert!(source.contains("sel!(applicationShouldTerminate:)"));
        assert!(source.contains("ClassBuilder::new("));
        assert!(source.contains("AnyObject::set_class(delegate, guarded_class)"));
        assert!(source.contains("NSApplicationTerminateReply::TerminateLater"));
        assert!(!source.contains(&["set", "Delegate"].concat()));
        assert!(!source.contains(&["add", "_ivar"].concat()));
    }
}
