use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    tauri_build::build();

    // Download a pdfium dynamic library for the current target into
    // resources/pdfium/ if not already present. The library is bundled into
    // the Tauri app via tauri.conf.json's bundle.resources and loaded at
    // runtime by PdfiumExtractor.
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
    println!("cargo:rerun-if-env-changed=CHRONACLE_SKIP_PDFIUM_DOWNLOAD");

    if lib_path.exists() {
        return;
    }

    fs::create_dir_all(&resources_dir).expect("create resources/pdfium dir");

    let url = format!(
        "https://github.com/bblanchon/pdfium-binaries/releases/latest/download/{asset}"
    );
    println!("cargo:warning=Downloading pdfium binary from {url}");

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .expect("build reqwest client");
    let resp = client.get(&url).send().expect("download pdfium");
    if !resp.status().is_success() {
        panic!("pdfium download failed: HTTP {}", resp.status());
    }
    let bytes = resp.bytes().expect("read pdfium body");

    let tar = flate2::read::GzDecoder::new(std::io::Cursor::new(bytes.as_ref()));
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
