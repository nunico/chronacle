use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[path = "src/runtime_downloads.rs"]
mod runtime_downloads;

fn main() {
    tauri_build::build();
    println!("cargo:rerun-if-env-changed=CHRONACLE_SKIP_RUNTIME_DOWNLOADS");
    println!("cargo:rerun-if-env-changed=CHRONACLE_SKIP_PDFIUM_DOWNLOAD");
    println!("cargo:rerun-if-env-changed=CHRONACLE_SKIP_ORT_DOWNLOAD");

    let decision = runtime_downloads::download_decision(
        env::var_os("CARGO_FEATURE_ROCKSDB").is_some(),
        env::var_os("CHRONACLE_SKIP_RUNTIME_DOWNLOADS").is_some(),
        env::var_os("CHRONACLE_SKIP_PDFIUM_DOWNLOAD").is_some(),
        env::var_os("CHRONACLE_SKIP_ORT_DOWNLOAD").is_some(),
    );
    if decision.pdfium {
        download_pdfium();
    }
    if decision.onnxruntime {
        download_onnxruntime();
    }
}

/// Download a pdfium dynamic library for the current target into
/// `resources/pdfium/` if not already present. The library is bundled into the
/// Tauri app via `tauri.conf.json`'s `bundle.resources` and loaded at runtime by
/// `PdfiumExtractor`.
fn download_pdfium() {
    if env::var("CHRONACLE_SKIP_PDFIUM_DOWNLOAD").is_ok() {
        println!("cargo:warning=CHRONACLE_SKIP_PDFIUM_DOWNLOAD set — skipping pdfium fetch.");
        return;
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    // bblanchon/pdfium-binaries release asset naming + the resulting lib name
    let (asset, lib_name) = match (target_os.as_str(), target_arch.as_str()) {
        ("macos", "aarch64") => ("pdfium-mac-arm64.tgz", "libpdfium.dylib"),
        ("macos", "x86_64") => ("pdfium-mac-x64.tgz", "libpdfium.dylib"),
        ("linux", "x86_64") => ("pdfium-linux-x64.tgz", "libpdfium.so"),
        ("linux", "aarch64") => ("pdfium-linux-arm64.tgz", "libpdfium.so"),
        ("windows", "x86_64") => ("pdfium-win-x64.tgz", "pdfium.dll"),
        _ => {
            println!(
                "cargo:warning=Unsupported target {target_os}/{target_arch} — pdfium not downloaded; runtime PDF extraction will fail."
            );
            return;
        }
    };

    let resources_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("resources/pdfium");
    let lib_path = resources_dir.join(lib_name);

    println!("cargo:rerun-if-changed=resources/pdfium/{lib_name}");

    if lib_path.exists() {
        return;
    }

    fs::create_dir_all(&resources_dir).expect("create resources/pdfium dir");

    let url =
        format!("https://github.com/bblanchon/pdfium-binaries/releases/latest/download/{asset}");
    println!("cargo:warning=Downloading pdfium binary from {url}");

    let bytes = http_get(&url, "pdfium");
    let tar = flate2::read::GzDecoder::new(std::io::Cursor::new(bytes.as_slice()));
    let mut archive = tar::Archive::new(tar);
    let mut found = false;
    for entry in archive.entries().expect("read tar") {
        let mut entry = entry.expect("entry");
        let path = entry.path().expect("entry path").into_owned();
        if path.file_name().and_then(|s| s.to_str()) == Some(lib_name) {
            let mut out = fs::File::create(&lib_path).expect("create pdfium lib file");
            std::io::copy(&mut entry, &mut out).expect("write pdfium lib");
            found = true;
            break;
        }
    }
    assert!(found, "pdfium binary {lib_name} not found in archive");
}

/// ONNX Runtime version to bundle. Must satisfy the `ort` crate's expected ABI
/// (`ort-sys` 2.0.0-rc.12 targets ONNX Runtime 1.24).
const ORT_VERSION: &str = "1.24.2";

/// Download the ONNX Runtime dynamic library for the current target into
/// `resources/onnxruntime/`. `fastembed` uses the `ort-load-dynamic` feature, so
/// the library is loaded at runtime (see `embedding.rs::ensure_ort_dylib_path`).
/// Bundled into the app via `tauri.conf.json`'s `bundle.resources`.
fn download_onnxruntime() {
    if env::var("CHRONACLE_SKIP_ORT_DOWNLOAD").is_ok() {
        println!("cargo:warning=CHRONACLE_SKIP_ORT_DOWNLOAD set — skipping ONNX Runtime fetch.");
        return;
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    // Microsoft ONNX Runtime release asset naming, archive kind, and the
    // resulting library name we extract to. Windows ships .zip; macOS/Linux .tgz.
    let (asset, kind, lib_name) = match (target_os.as_str(), target_arch.as_str()) {
        ("macos", "aarch64") => (
            format!("onnxruntime-osx-arm64-{ORT_VERSION}.tgz"),
            Archive::Tar,
            "libonnxruntime.dylib",
        ),
        ("linux", "x86_64") => (
            format!("onnxruntime-linux-x64-{ORT_VERSION}.tgz"),
            Archive::Tar,
            "libonnxruntime.so",
        ),
        ("linux", "aarch64") => (
            format!("onnxruntime-linux-aarch64-{ORT_VERSION}.tgz"),
            Archive::Tar,
            "libonnxruntime.so",
        ),
        ("windows", "x86_64") => (
            format!("onnxruntime-win-x64-{ORT_VERSION}.zip"),
            Archive::Zip,
            "onnxruntime.dll",
        ),
        ("windows", "aarch64") => (
            format!("onnxruntime-win-arm64-{ORT_VERSION}.zip"),
            Archive::Zip,
            "onnxruntime.dll",
        ),
        // Microsoft does not publish a macOS x86_64 build for ONNX Runtime 1.24
        // (Intel Mac support was dropped). Embeddings are unavailable there.
        _ => {
            println!(
                "cargo:warning=No ONNX Runtime {ORT_VERSION} binary for {target_os}/{target_arch} — local embeddings will be unavailable on this target."
            );
            return;
        }
    };

    let resources_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("resources/onnxruntime");
    let lib_path = resources_dir.join(lib_name);

    println!("cargo:rerun-if-changed=resources/onnxruntime/{lib_name}");

    if lib_path.exists() {
        return;
    }

    fs::create_dir_all(&resources_dir).expect("create resources/onnxruntime dir");

    let url = format!(
        "https://github.com/microsoft/onnxruntime/releases/download/v{ORT_VERSION}/{asset}"
    );
    println!("cargo:warning=Downloading ONNX Runtime binary from {url}");

    let bytes = http_get(&url, "ONNX Runtime");
    let lib_bytes = match kind {
        Archive::Tar => extract_lib_from_tar(&bytes, target_os.as_str()),
        Archive::Zip => extract_lib_from_zip(&bytes, target_os.as_str()),
    };
    let lib_bytes = lib_bytes.unwrap_or_else(|| {
        panic!("ONNX Runtime library ({lib_name}) not found in archive {asset}")
    });
    fs::write(&lib_path, lib_bytes).expect("write ONNX Runtime lib");
}

enum Archive {
    Tar,
    Zip,
}

/// Does this archive entry look like the main ONNX Runtime shared library for
/// `target_os`? Skips debug symbols (`.dSYM`) and the separate
/// `*_providers_shared` helper; the largest-wins rule in the callers then picks
/// the real binary over any zero-byte version symlink.
fn is_ort_lib(target_os: &str, full_path: &str, file_name: &str) -> bool {
    if full_path.contains(".dSYM") || file_name.contains("providers") {
        return false;
    }
    if !file_name.contains("onnxruntime") {
        return false;
    }
    match target_os {
        "macos" => file_name.ends_with(".dylib"),
        "windows" => file_name.ends_with(".dll"),
        // Linux ships the real file as `libonnxruntime.so.<version>`.
        _ => file_name.contains(".so"),
    }
}

/// Extract the largest matching ONNX Runtime library from a gzipped tar. Largest
/// wins so the real Mach-O/ELF binary is chosen over zero-byte version symlinks.
fn extract_lib_from_tar(bytes: &[u8], target_os: &str) -> Option<Vec<u8>> {
    let tar = flate2::read::GzDecoder::new(std::io::Cursor::new(bytes));
    let mut archive = tar::Archive::new(tar);
    let mut best: Option<Vec<u8>> = None;
    for entry in archive.entries().expect("read ort tar") {
        let mut entry = entry.expect("ort entry");
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().expect("ort entry path").into_owned();
        let full = path.to_string_lossy().to_string();
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !is_ort_lib(target_os, &full, &name) {
            continue;
        }
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).expect("read ort entry bytes");
        if best.as_ref().is_none_or(|b| buf.len() > b.len()) {
            best = Some(buf);
        }
    }
    best
}

/// Extract the largest matching ONNX Runtime library from a zip archive.
fn extract_lib_from_zip(bytes: &[u8], target_os: &str) -> Option<Vec<u8>> {
    let reader = std::io::Cursor::new(bytes.to_vec());
    let mut zip = zip::ZipArchive::new(reader).expect("read ort zip");
    let mut best: Option<Vec<u8>> = None;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).expect("zip entry");
        if !file.is_file() {
            continue;
        }
        let full = file.name().to_string();
        let name = Path::new(&full)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if !is_ort_lib(target_os, &full, &name) {
            continue;
        }
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).expect("read zip entry bytes");
        if best.as_ref().is_none_or(|b| buf.len() > b.len()) {
            best = Some(buf);
        }
    }
    best
}

/// Blocking HTTP GET that follows redirects (GitHub release downloads redirect to
/// a CDN). Panics with a clear message on failure — a missing native library is a
/// hard build error.
fn http_get(url: &str, what: &str) -> Vec<u8> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .expect("build reqwest client");
    let resp = client
        .get(url)
        .send()
        .unwrap_or_else(|e| panic!("download {what} failed: {e}"));
    if !resp.status().is_success() {
        panic!("{what} download failed: HTTP {}", resp.status());
    }
    resp.bytes()
        .unwrap_or_else(|e| panic!("read {what} body failed: {e}"))
        .to_vec()
}
