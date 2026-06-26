/// Chunker — splits extracted PDF text into searchable chunks.
///
/// Pipeline: section detection → sliding-window split
///
/// - **Section detector**: identifies headings via regex patterns common in
///   TTRPG rulebooks (ALL CAPS, "Chapter X", numbered sections).
/// - **Sliding window**: ~400 tokens per chunk with ~80-token overlap.
///   Token count is approximated as `chars / 4` (reasonable for English text).
///
/// Chunks respect section boundaries: when a section break falls within the
/// overlap region, the chunk is split at the heading instead.
mod core;
mod heading;
mod types;

pub use core::{approx_token_count, chunk_document};
pub use heading::{is_heading, is_title_case_heading};
pub use types::{Chunk, ExtractedDoc, PageContent};

#[cfg(test)]
mod tests_heading;
#[cfg(test)]
mod tests_chunking;
