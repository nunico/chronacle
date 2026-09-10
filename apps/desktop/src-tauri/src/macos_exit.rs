use crate::exit_guard::ExitAuthorization;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeTerminationReply {
    Later,
    Cancel,
}

fn intercept_native_exit(
    authorization: &ExitAuthorization,
    publish: impl FnOnce(u64) -> Result<(), ()>,
) -> NativeTerminationReply {
    let request = authorization.intercept_native_exit();
    if request.should_emit() && publish(request.intent()).is_err() {
        authorization.rollback_native_request(request.intent());
        return NativeTerminationReply::Cancel;
    }
    NativeTerminationReply::Later
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
                    app.emit_to("main", "app-exit-requested", ExitRequestPayload { intent })
                        .map_err(|error| {
                            eprintln!("failed to publish macOS application exit request: {error}");
                        })
                })
            })
            .unwrap_or(NativeTerminationReply::Cancel);

        match reply {
            NativeTerminationReply::Later => NSApplicationTerminateReply::TerminateLater,
            NativeTerminationReply::Cancel => NSApplicationTerminateReply::TerminateCancel,
        }
    }

    pub(crate) fn schedule_reply(
        app: &tauri::AppHandle,
        intent: u64,
        terminate: bool,
    ) -> Result<(), String> {
        let app_handle = app.clone();
        app.run_on_main_thread(move || {
            let Some(main_thread) = objc2::MainThreadMarker::new() else {
                let authorization = app_handle.state::<ExitAuthorization>();
                authorization.rollback_native_resolution(intent, terminate);
                eprintln!("macOS exit reply did not run on the main thread");
                return;
            };
            let application = NSApplication::sharedApplication(main_thread);
            application.replyToApplicationShouldTerminate(terminate);
            let authorization = app_handle.state::<ExitAuthorization>();
            if !authorization.complete_native_reply(intent, terminate) {
                eprintln!("macOS exit reply state changed before completion");
            }
        })
        .map_err(|error| format!("Could not schedule the macOS close reply: {error}"))
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
#[cfg(all(target_os = "macos", feature = "rocksdb"))]
pub(crate) use platform::schedule_reply;

#[cfg(test)]
mod tests {
    use super::{intercept_native_exit, NativeTerminationReply};
    use crate::exit_guard::ExitAuthorization;

    #[test]
    fn unauthorized_native_quit_is_deferred_and_publishes_the_shared_intent() {
        let authorization = ExitAuthorization::default();
        let mut published = Vec::new();

        let reply = intercept_native_exit(&authorization, |intent| {
            published.push(intent);
            Ok(())
        });

        assert_eq!(reply, NativeTerminationReply::Later);
        assert_eq!(published, vec![authorization.pending().unwrap()]);
    }

    #[test]
    fn repeated_native_quit_requests_coalesce() {
        let authorization = ExitAuthorization::default();
        let mut published = Vec::new();

        assert_eq!(
            intercept_native_exit(&authorization, |intent| {
                published.push(intent);
                Ok(())
            }),
            NativeTerminationReply::Later
        );
        assert_eq!(
            intercept_native_exit(&authorization, |intent| {
                published.push(intent);
                Ok(())
            }),
            NativeTerminationReply::Later
        );

        assert_eq!(published.len(), 1);
    }

    #[test]
    fn failed_emission_cancels_instead_of_leaving_appkit_deferred() {
        let authorization = ExitAuthorization::default();

        let reply = intercept_native_exit(&authorization, |_intent| Err(()));

        assert_eq!(reply, NativeTerminationReply::Cancel);
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
        assert!(source.contains("NSApplicationTerminateReply::TerminateCancel"));
        assert!(source.contains("replyToApplicationShouldTerminate(terminate)"));
        assert!(source.contains("run_on_main_thread"));
        assert!(source.contains("rollback_native_request"));
        assert!(!source.contains(&["set", "Delegate"].concat()));
        assert!(!source.contains(&["add", "_ivar"].concat()));
    }
}
