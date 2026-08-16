#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DownloadDecision {
    pub(crate) pdfium: bool,
    pub(crate) onnxruntime: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ArchiveKind {
    Tar,
    Zip,
}

pub(crate) fn pdfium_asset(
    target_os: &str,
    target_arch: &str,
) -> Option<(&'static str, &'static str)> {
    match (target_os, target_arch) {
        ("macos", "aarch64") => Some(("pdfium-mac-arm64.tgz", "libpdfium.dylib")),
        ("macos", "x86_64") => Some(("pdfium-mac-x64.tgz", "libpdfium.dylib")),
        ("linux", "x86_64") => Some(("pdfium-linux-x64.tgz", "libpdfium.so")),
        ("linux", "aarch64") => Some(("pdfium-linux-arm64.tgz", "libpdfium.so")),
        ("windows", "x86_64") => Some(("pdfium-win-x64.tgz", "pdfium.dll")),
        _ => None,
    }
}

pub(crate) fn onnxruntime_asset(
    target_os: &str,
    target_arch: &str,
    version: &str,
) -> Option<(String, ArchiveKind, &'static str)> {
    match (target_os, target_arch) {
        ("macos", "aarch64") => Some((
            format!("onnxruntime-osx-arm64-{version}.tgz"),
            ArchiveKind::Tar,
            "libonnxruntime.dylib",
        )),
        ("linux", "x86_64") => Some((
            format!("onnxruntime-linux-x64-{version}.tgz"),
            ArchiveKind::Tar,
            "libonnxruntime.so",
        )),
        ("linux", "aarch64") => Some((
            format!("onnxruntime-linux-aarch64-{version}.tgz"),
            ArchiveKind::Tar,
            "libonnxruntime.so",
        )),
        ("windows", "x86_64") => Some((
            format!("onnxruntime-win-x64-{version}.zip"),
            ArchiveKind::Zip,
            "onnxruntime.dll",
        )),
        ("windows", "aarch64") => Some((
            format!("onnxruntime-win-arm64-{version}.zip"),
            ArchiveKind::Zip,
            "onnxruntime.dll",
        )),
        _ => None,
    }
}

pub(crate) fn download_decision(
    runnable_rocksdb_build: bool,
    skip_all: bool,
    skip_pdfium: bool,
    skip_onnxruntime: bool,
) -> DownloadDecision {
    DownloadDecision {
        pdfium: runnable_rocksdb_build && !skip_all && !skip_pdfium,
        onnxruntime: runnable_rocksdb_build && !skip_all && !skip_onnxruntime,
    }
}

#[cfg(test)]
#[path = "runtime_downloads_tests.rs"]
mod tests;
