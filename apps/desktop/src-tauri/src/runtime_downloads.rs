#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DownloadDecision {
    pub(crate) pdfium: bool,
    pub(crate) onnxruntime: bool,
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
