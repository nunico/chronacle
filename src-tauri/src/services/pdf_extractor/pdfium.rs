use async_trait::async_trait;

use crate::services::chunker::{ExtractedDoc, PageContent};

use super::types::{PageProgressFn, PdfExtractError, PdfExtractor};

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
/// font OR mostly-bold) in paragraph breaks so the chunker's `is_heading`
/// heuristics treat them as their own section.
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
            Some(CharInfo { ch, font_size, is_bold, y })
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
            if !out.is_empty() && !out.ends_with("\n\n") {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push('\n');
            }
            out.push_str(trimmed);
            out.push_str("\n\n");
        } else {
            out.push_str(&line_text);
            out.push('\n');
        }
    }

    Ok(out)
}
