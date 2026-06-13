//! PDF text extraction abstraction.
//!
//! Backed by `pdfium-render` (Chromium's PDF engine) for layout-aware
//! extraction that handles multi-column TTRPG rulebooks correctly. The
//! library is loaded at runtime from a bundled binary; see `build.rs`.

use std::sync::Arc;

use async_trait::async_trait;

use crate::services::chunker::{ExtractedDoc, PageContent};

/// Callback invoked once per PDF page during extraction: `(page_num, total)`,
/// 1-based page number and total page count. Held behind an `Arc` so it can be
/// moved into the blocking extraction task (which requires `'static + Send`).
pub type PageProgressFn = Arc<dyn Fn(usize, usize) + Send + Sync>;

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
    /// Extract one [`PageContent`] per PDF page, reporting per-page progress.
    ///
    /// `on_page` is invoked after each page is processed with `(page_num, total)`.
    /// This is the stage's only countable unit of work, so reporting it lets the
    /// UI show steady activity through large rulebooks.
    async fn extract_with_progress(
        &self,
        data: &[u8],
        on_page: PageProgressFn,
    ) -> Result<ExtractedDoc, PdfExtractError>;

    /// Extract one [`PageContent`] per PDF page (no progress reporting).
    ///
    /// Convenience wrapper over [`extract_with_progress`](Self::extract_with_progress)
    /// for callers that don't need per-page updates (tests, one-off extractions).
    async fn extract(&self, data: &[u8]) -> Result<ExtractedDoc, PdfExtractError> {
        self.extract_with_progress(data, Arc::new(|_, _| {})).await
    }
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
    async fn extract_with_progress(
        &self,
        data: &[u8],
        on_page: PageProgressFn,
    ) -> Result<ExtractedDoc, PdfExtractError> {
        let data = data.to_vec();
        let lib_path = self.library_path.clone();
        tokio::task::spawn_blocking(move || extract_blocking(&lib_path, &data, on_page.as_ref()))
            .await
            .map_err(|e| PdfExtractError::Parse(format!("join error: {e}")))?
    }
}

fn extract_blocking(
    library_path: &std::path::Path,
    data: &[u8],
    on_page: &(dyn Fn(usize, usize) + Send + Sync),
) -> Result<ExtractedDoc, PdfExtractError> {
    use pdfium_render::prelude::*;

    let bindings = Pdfium::bind_to_library(library_path)
        .map_err(|e| PdfExtractError::LibLoad(e.to_string()))?;
    let pdfium = Pdfium::new(bindings);

    let document = pdfium
        .load_pdf_from_byte_slice(data, None)
        .map_err(|e| PdfExtractError::Parse(e.to_string()))?;

    let total = document.pages().len() as usize;
    let mut pages = Vec::new();
    let mut full = String::new();
    for (i, page) in document.pages().iter().enumerate() {
        let text = extract_page_text_with_heading_breaks(&page)?;
        if i > 0 && !text.is_empty() && !full.is_empty() {
            full.push('\n');
        }
        full.push_str(&text);
        pages.push(PageContent {
            page_num: i + 1,
            text,
        });
        on_page(i + 1, total);
    }

    Ok(ExtractedDoc {
        page_count: pages.len(),
        text: full,
        pages,
    })
}

/// Per-character metadata collected from a PDF page.
struct CharInfo {
    ch: char,
    font_size: f32,
    is_bold: bool,
    y: f32,
}

/// Extract one PDF page's text and wrap visually-distinct headings (larger
/// font OR mostly-bold) in paragraph breaks (`\n\n`) so the downstream
/// chunker / `is_heading` heuristics treat them as their own section.
///
/// This catches headings that the text-pattern heuristics miss — e.g. a
/// Title-Case heading set in body case but a larger/bolder font.
fn extract_page_text_with_heading_breaks(
    page: &pdfium_render::prelude::PdfPage,
) -> Result<String, PdfExtractError> {
    use pdfium_render::prelude::PdfFontWeight;

    let text = page
        .text()
        .map_err(|e| PdfExtractError::Parse(e.to_string()))?;

    let chars: Vec<CharInfo> = text
        .chars()
        .iter()
        .filter_map(|c| {
            let ch = c.unicode_char()?;
            let font_size = c.scaled_font_size().value;
            let is_bold = matches!(
                c.font_weight(),
                Some(
                    PdfFontWeight::Weight700Bold
                        | PdfFontWeight::Weight800
                        | PdfFontWeight::Weight900
                )
            );
            let y = c.origin_y().map(|p| p.value).unwrap_or(0.0);
            Some(CharInfo {
                ch,
                font_size,
                is_bold,
                y,
            })
        })
        .collect();

    if chars.is_empty() {
        return Ok(String::new());
    }

    // Body font size = mode (most common 0.5pt bucket). Headings beat this.
    let mut size_counts: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for c in &chars {
        let key = (c.font_size * 2.0).round() as u32;
        *size_counts.entry(key).or_insert(0) += 1;
    }
    let body_size_key = size_counts
        .iter()
        .max_by_key(|(_, n)| **n)
        .map(|(k, _)| *k)
        .unwrap_or(20);
    let body_size = body_size_key as f32 / 2.0;

    // Group chars into visual lines. pdfium emits chars in reading order
    // but doesn't insert \n; we split where Y jumps by ≥ half the body
    // font size. Explicit \n / \r chars also flush the current line.
    let mut lines: Vec<Vec<&CharInfo>> = Vec::new();
    let mut cur: Vec<&CharInfo> = Vec::new();
    let mut prev_y: Option<f32> = None;
    for c in &chars {
        if c.ch == '\n' || c.ch == '\r' {
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
            }
            prev_y = None;
            continue;
        }
        let new_line = match prev_y {
            Some(py) => (c.y - py).abs() > body_size * 0.5,
            None => false,
        };
        if new_line && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
        }
        cur.push(c);
        prev_y = Some(c.y);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }

    // Build output, wrapping styled-heading lines in paragraph breaks so
    // text_normalizer (which collapses single \n to space but preserves
    // \n\n) keeps them on their own line for the chunker.
    let mut out = String::new();
    for line in lines {
        let line_text: String = line.iter().map(|c| c.ch).collect();
        let trimmed = line_text.trim();
        if trimmed.is_empty() {
            // Preserve blank-line paragraph breaks
            if !out.ends_with("\n\n") {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push('\n');
            }
            continue;
        }
        let n = line.len() as f32;
        let avg_size: f32 = line.iter().map(|c| c.font_size).sum::<f32>() / n;
        let bold_ratio: f32 = line.iter().filter(|c| c.is_bold).count() as f32 / n;
        let is_styled_heading = avg_size > body_size * 1.15 || bold_ratio >= 0.6;

        if is_styled_heading {
            // Paragraph break before the heading
            if !out.is_empty() && !out.ends_with("\n\n") {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push('\n');
            }
            out.push_str(trimmed);
            // And after
            out.push_str("\n\n");
        } else {
            out.push_str(&line_text);
            out.push('\n');
        }
    }

    Ok(out)
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

    /// Build a PDF with one page per supplied text run.
    fn make_pdf_with_pages(texts: &[&str]) -> Vec<u8> {
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

        let mut kids: Vec<Object> = Vec::new();
        for text in texts {
            let content = Content {
                operations: vec![
                    Operation::new("BT", vec![]),
                    Operation::new("Tf", vec!["F1".into(), 24.into()]),
                    Operation::new("Td", vec![100.into(), 600.into()]),
                    Operation::new("Tj", vec![Object::string_literal(*text)]),
                    Operation::new("ET", vec![]),
                ],
            };
            let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
            });
            kids.push(page_id.into());
        }

        let count = kids.len() as i64;
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => count,
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
        let on_page: PageProgressFn =
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

    /// Build a 1-page PDF with two text runs at different font sizes,
    /// stacked vertically. Used to verify style-based heading detection.
    fn make_pdf_with_styled_heading(heading: &str, body: &str) -> Vec<u8> {
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
        // Heading: 24pt at y=700; body: 10pt at y=650.
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 24.into()]),
                Operation::new("Td", vec![100.into(), 700.into()]),
                Operation::new("Tj", vec![Object::string_literal(heading)]),
                Operation::new("ET", vec![]),
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 10.into()]),
                Operation::new("Td", vec![100.into(), 650.into()]),
                Operation::new("Tj", vec![Object::string_literal(body)]),
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

    /// Regression: a heading set in a clearly larger font should end up on
    /// its own line wrapped in paragraph breaks, even though the heading
    /// text is Title-Case (not ALL-CAPS) and pdfium would otherwise emit
    /// it on the same logical line as body text.
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
}
