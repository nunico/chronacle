/// A single chunk produced by the chunker.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub text: String,
    pub page_start: i64,
    pub page_end: i64,
    pub section_heading: String,
}

/// Extracted page content fed into the chunker.
#[derive(Debug, Clone)]
pub struct PageContent {
    pub page_num: usize,
    pub text: String,
}

/// A document ready for chunking.
#[derive(Debug, Clone)]
pub struct ExtractedDoc {
    pub page_count: usize,
    pub text: String,
    pub pages: Vec<PageContent>,
}
