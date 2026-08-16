use super::*;

#[test]
fn downloads_only_for_runnable_rocksdb_builds() {
    assert_eq!(
        download_decision(false, false, false, false),
        DownloadDecision {
            pdfium: false,
            onnxruntime: false,
        }
    );
    assert_eq!(
        download_decision(true, false, false, false),
        DownloadDecision {
            pdfium: true,
            onnxruntime: true,
        }
    );
}

#[test]
fn global_and_resource_specific_skips_are_honored() {
    assert_eq!(
        download_decision(true, true, false, false),
        DownloadDecision {
            pdfium: false,
            onnxruntime: false,
        }
    );
    assert_eq!(
        download_decision(true, false, true, false),
        DownloadDecision {
            pdfium: false,
            onnxruntime: true,
        }
    );
    assert_eq!(
        download_decision(true, false, false, true),
        DownloadDecision {
            pdfium: true,
            onnxruntime: false,
        }
    );
}

#[test]
fn selects_pdfium_assets_for_supported_linux_architectures() {
    assert_eq!(
        pdfium_asset("linux", "x86_64"),
        Some(("pdfium-linux-x64.tgz", "libpdfium.so"))
    );
    assert_eq!(
        pdfium_asset("linux", "aarch64"),
        Some(("pdfium-linux-arm64.tgz", "libpdfium.so"))
    );
}

#[test]
fn selects_onnxruntime_assets_for_supported_linux_architectures() {
    assert_eq!(
        onnxruntime_asset("linux", "x86_64", "1.24.2"),
        Some((
            "onnxruntime-linux-x64-1.24.2.tgz".to_string(),
            ArchiveKind::Tar,
            "libonnxruntime.so",
        ))
    );
    assert_eq!(
        onnxruntime_asset("linux", "aarch64", "1.24.2"),
        Some((
            "onnxruntime-linux-aarch64-1.24.2.tgz".to_string(),
            ArchiveKind::Tar,
            "libonnxruntime.so",
        ))
    );
}

#[test]
fn selects_pdfium_but_not_onnxruntime_for_intel_macos() {
    assert_eq!(
        pdfium_asset("macos", "x86_64"),
        Some(("pdfium-mac-x64.tgz", "libpdfium.dylib"))
    );
    assert_eq!(onnxruntime_asset("macos", "x86_64", "1.24.2"), None);
}
