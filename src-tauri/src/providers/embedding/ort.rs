/// ONNX Runtime dynamic library path resolution.
///
/// `fastembed` is built with `ort-load-dynamic`, so ONNX Runtime is loaded at
/// runtime. These helpers locate the right library for the current platform,
/// checking the bundled resource first then a system/Homebrew install.
use std::path::{Path, PathBuf};

/// Platform-specific filename of the bundled ONNX Runtime dynamic library.
const ORT_DYLIB_NAME: &str = if cfg!(target_os = "macos") {
    "libonnxruntime.dylib"
} else if cfg!(target_os = "windows") {
    "onnxruntime.dll"
} else {
    "libonnxruntime.so"
};

fn onnxruntime_library_path() -> Option<PathBuf> {
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources/onnxruntime")
        .join(ORT_DYLIB_NAME);
    if dev.exists() {
        return Some(dev);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let mac = exe_dir
                .join("../Resources/resources/onnxruntime")
                .join(ORT_DYLIB_NAME);
            if mac.exists() {
                return Some(mac);
            }
            let other = exe_dir.join("resources/onnxruntime").join(ORT_DYLIB_NAME);
            if other.exists() {
                return Some(other);
            }
        }
    }
    None
}

/// Locate a system- or Homebrew-installed ONNX Runtime library.
///
/// Fallback for targets with no bundled binary (notably macOS x86_64, which
/// Microsoft no longer ships). Also picks up an Apple-Silicon Homebrew install
/// on dev machines. The library is unpinned — relies on ONNX Runtime ABI
/// forward-compatibility (`GetApi(N)` succeeds on any runtime ≥ N).
fn system_onnxruntime_library_path() -> Option<PathBuf> {
    const SEARCH_DIRS: &[&str] = &[
        "/opt/homebrew/lib",
        "/usr/local/lib",
        "/home/linuxbrew/.linuxbrew/lib",
        "/usr/lib",
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
    ];
    SEARCH_DIRS
        .iter()
        .map(|dir| Path::new(dir).join(ORT_DYLIB_NAME))
        .find(|p| p.exists())
}

/// Bundled resource first (version-controlled), then system/Homebrew install.
pub(super) fn resolve_onnxruntime_library_path() -> Option<PathBuf> {
    onnxruntime_library_path().or_else(system_onnxruntime_library_path)
}

/// Point `ort` at an ONNX Runtime library before any session is built.
///
/// `ort` reads `ORT_DYLIB_PATH` once, lazily, when the first inference session
/// is created. Safe to call repeatedly — no-op once the variable is set.
pub(super) fn ensure_ort_dylib_path() {
    if std::env::var_os("ORT_DYLIB_PATH").is_some() {
        return;
    }
    if let Some(path) = resolve_onnxruntime_library_path() {
        std::env::set_var("ORT_DYLIB_PATH", path);
    }
}

/// Returns `true` if an ONNX Runtime library is available for this platform.
///
/// Returns `false` on targets with neither a bundled binary nor a system
/// install (notably stock macOS x86_64). The UI uses this to steer such users
/// toward a cloud embedding backend.
pub fn local_embeddings_available() -> bool {
    resolve_onnxruntime_library_path().is_some()
}

#[cfg(test)]
mod tests {
    use super::ORT_DYLIB_NAME;

    #[test]
    fn ort_dylib_name_matches_target_platform() {
        if cfg!(target_os = "macos") {
            assert!(ORT_DYLIB_NAME.ends_with(".dylib"));
        } else if cfg!(target_os = "windows") {
            assert!(ORT_DYLIB_NAME.ends_with(".dll"));
        } else {
            assert!(ORT_DYLIB_NAME.ends_with(".so"));
        }
    }
}
