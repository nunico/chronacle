use std::sync::Arc;

use async_trait::async_trait;

use crate::services::chunker::ExtractedDoc;

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
    async fn extract_with_progress(
        &self,
        data: &[u8],
        on_page: PageProgressFn,
    ) -> Result<ExtractedDoc, PdfExtractError>;

    /// Extract one [`PageContent`] per PDF page (no progress reporting).
    async fn extract(&self, data: &[u8]) -> Result<ExtractedDoc, PdfExtractError> {
        self.extract_with_progress(data, Arc::new(|_, _| {})).await
    }
}
