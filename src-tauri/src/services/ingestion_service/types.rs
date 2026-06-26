/// A progress update emitted during ingestion.
///
/// `fraction` advances from 0.0 to 1.0 across all stages.
/// `step` is a human-readable label like "Extracting text from PDF".
/// `current`/`total` carry item counts for batched stages (e.g. embedding
/// "64 of 120 chunks") so the UI can show fine-grained activity. They are
/// `None` for single-shot stages that have no countable unit of work.
#[derive(Debug, Clone)]
pub struct IngestionProgress {
    pub fraction: f32,
    pub step: String,
    pub current: Option<u32>,
    pub total: Option<u32>,
}

impl IngestionProgress {
    /// A single-shot stage with no countable work (extraction, DB write, …).
    pub(super) fn stage(fraction: f32, step: impl Into<String>) -> Self {
        Self {
            fraction,
            step: step.into(),
            current: None,
            total: None,
        }
    }

    /// A batched stage reporting `current`/`total` items processed so far.
    pub(super) fn counted(fraction: f32, step: impl Into<String>, current: u32, total: u32) -> Self {
        Self {
            fraction,
            step: step.into(),
            current: Some(current),
            total: Some(total),
        }
    }
}

/// Errors that can arise during ingestion.
#[derive(Debug, thiserror::Error)]
pub enum IngestionError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("PDF extraction error: {0}")]
    PdfExtraction(String),
    #[error("Embedding error: {0}")]
    Embedding(String),
    #[error("Database error: {0}")]
    Db(String),
    #[error("Store error: {0}")]
    Store(String),
}

/// A raw chunk produced by the chunker, before embedding.
pub(super) struct RawChunk {
    pub(super) text: String,
    pub(super) page_start: i64,
    pub(super) page_end: i64,
    pub(super) section_heading: String,
}

/// Information about a source needed during ingestion.
#[derive(Debug)]
pub(crate) struct SourceInfo {
    pub(crate) filename: String,
    /// Every source must belong to a collection — non-nullable per schema.
    pub(crate) collection_id: String,
}
