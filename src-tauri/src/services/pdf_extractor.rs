//! PDF text extraction abstraction.
//!
//! Backed by `pdfium-render` (Chromium's PDF engine) for layout-aware
//! extraction that handles multi-column TTRPG rulebooks correctly. The
//! library is loaded at runtime from a bundled binary; see `build.rs`.

use async_trait::async_trait;

use crate::services::chunker::{ExtractedDoc, PageContent};

/// Errors raised by [`PdfExtractor`] implementations.
#[derive(Debug, thiserror::Error)]
pub enum PdfExtractError {
    #[error("PDF library load failed: {0}")]
    LibLoad(String),
    #[error("PDF parse failed: {0}")]
    Parse(String),
}

/// Trait for extracting text + page structure from PDF bytes.
///
/// Implementations MUST be `Send + Sync` so they can live behind an
/// `Arc<dyn PdfExtractor>` in `AppState`.
#[async_trait]
pub trait PdfExtractor: Send + Sync {
    /// Extract one [`PageContent`] per PDF page.
    async fn extract(&self, data: &[u8]) -> Result<ExtractedDoc, PdfExtractError>;
}

/// Pdfium-backed implementation. Binds to the dylib at `library_path` on
/// first call (cached internally by pdfium-render).
pub struct PdfiumExtractor {
    library_path: std::path::PathBuf,
}

impl PdfiumExtractor {
    pub fn new(library_path: std::path::PathBuf) -> Self {
        Self { library_path }
    }
}

#[async_trait]
impl PdfExtractor for PdfiumExtractor {
    async fn extract(&self, data: &[u8]) -> Result<ExtractedDoc, PdfExtractError> {
        let data = data.to_vec();
        let lib_path = self.library_path.clone();
        tokio::task::spawn_blocking(move || extract_blocking(&lib_path, &data))
            .await
            .map_err(|e| PdfExtractError::Parse(format!("join error: {e}")))?
    }
}

fn extract_blocking(
    library_path: &std::path::Path,
    data: &[u8],
) -> Result<ExtractedDoc, PdfExtractError> {
    use pdfium_render::prelude::*;

    let bindings = Pdfium::bind_to_library(library_path)
        .map_err(|e| PdfExtractError::LibLoad(e.to_string()))?;
    let pdfium = Pdfium::new(bindings);

    let document = pdfium
        .load_pdf_from_byte_slice(data, None)
        .map_err(|e| PdfExtractError::Parse(e.to_string()))?;

    let mut pages = Vec::new();
    let mut full = String::new();
    for (i, page) in document.pages().iter().enumerate() {
        let text = page
            .text()
            .map_err(|e| PdfExtractError::Parse(e.to_string()))?
            .all();
        if i > 0 && !text.is_empty() && !full.is_empty() {
            full.push('\n');
        }
        full.push_str(&text);
        pages.push(PageContent {
            page_num: i + 1,
            text,
        });
    }

    Ok(ExtractedDoc {
        page_count: pages.len(),
        text: full,
        pages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pdfium_lib_path() -> std::path::PathBuf {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium");
        let name = if cfg!(target_os = "macos") {
            "libpdfium.dylib"
        } else if cfg!(target_os = "linux") {
            "libpdfium.so"
        } else {
            "pdfium.dll"
        };
        dir.join(name)
    }

    fn make_one_page_pdf(text: &str) -> Vec<u8> {
        use lopdf::content::{Content, Operation};
        use lopdf::dictionary;
        use lopdf::{Document, Object, Stream};

        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 24.into()]),
                Operation::new("Td", vec![100.into(), 600.into()]),
                Operation::new("Tj", vec![Object::string_literal(text)]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
        });
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[tokio::test]
    async fn extracts_text_from_minimal_pdf() {
        let lib = pdfium_lib_path();
        if !lib.exists() {
            eprintln!("Skipping — pdfium binary not present at {lib:?}");
            return;
        }
        let pdf = make_one_page_pdf("Coriolis orbits Kua");
        let extractor = PdfiumExtractor::new(lib);
        let doc = extractor.extract(&pdf).await.expect("extract");
        assert_eq!(doc.page_count, 1);
        assert!(
            doc.text.contains("Coriolis") && doc.text.contains("Kua"),
            "extracted text missing markers: {:?}",
            doc.text
        );
    }
}
