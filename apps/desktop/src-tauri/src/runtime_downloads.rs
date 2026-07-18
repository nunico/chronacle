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
mod tests {
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
}
