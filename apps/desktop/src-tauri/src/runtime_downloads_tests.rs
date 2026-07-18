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
