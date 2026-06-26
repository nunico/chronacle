use std::sync::Arc;

use super::pdfium::PdfiumExtractor;
use super::test_builders::{
    make_one_page_pdf, make_pdf_with_pages, make_pdf_with_styled_heading, pdfium_lib_path,
};
use super::types::PdfExtractor;

#[tokio::test]
async fn extract_with_progress_reports_each_page() {
    use std::sync::Mutex;

    let lib = pdfium_lib_path();
    if !lib.exists() {
        eprintln!("Skipping — pdfium binary not present at {lib:?}");
        return;
    }
    let pdf = make_pdf_with_pages(&["Alpha page", "Beta page", "Gamma page"]);
    let extractor = PdfiumExtractor::new(lib);

    let calls = Arc::new(Mutex::new(Vec::<(usize, usize)>::new()));
    let captured = calls.clone();
    let on_page: super::types::PageProgressFn =
        Arc::new(move |page, total| captured.lock().unwrap().push((page, total)));

    let doc = extractor
        .extract_with_progress(&pdf, on_page)
        .await
        .expect("extract");

    assert_eq!(doc.page_count, 3);
    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.as_slice(),
        &[(1, 3), (2, 3), (3, 3)],
        "expected one progress callback per page with the full total"
    );
}

#[tokio::test]
async fn extracts_text_from_minimal_pdf() {
    let lib = pdfium_lib_path();
    if !lib.exists() {
        eprintln!("Skipping — pdfium binary not present at {lib:?}");
        return;
    }
    let pdf = make_one_page_pdf("Lantern orbits Mirovia");
    let extractor = PdfiumExtractor::new(lib);
    let doc = extractor.extract(&pdf).await.expect("extract");
    assert_eq!(doc.page_count, 1);
    assert!(
        doc.text.contains("Lantern") && doc.text.contains("Mirovia"),
        "extracted text missing markers: {:?}",
        doc.text
    );
}

/// Regression: a heading set in a clearly larger font should end up on
/// its own line wrapped in paragraph breaks, even though the heading
/// text is Title-Case (not ALL-CAPS).
#[tokio::test]
async fn styled_heading_gets_wrapped_in_paragraph_breaks() {
    let lib = pdfium_lib_path();
    if !lib.exists() {
        eprintln!("Skipping — pdfium binary not present at {lib:?}");
        return;
    }
    let pdf = make_pdf_with_styled_heading(
        "Lantern and Mirovia",
        "The center of the Ember Reach is the Velmar system.",
    );
    let extractor = PdfiumExtractor::new(lib);
    let doc = extractor.extract(&pdf).await.expect("extract");
    assert!(
        doc.text.contains("Lantern and Mirovia"),
        "extracted text missing heading: {:?}",
        doc.text
    );
    assert!(
        doc.text.contains("\n\nLantern and Mirovia\n\n")
            || doc.text.starts_with("Lantern and Mirovia\n\n"),
        "heading should be wrapped in paragraph breaks; got: {:?}",
        doc.text
    );
}
