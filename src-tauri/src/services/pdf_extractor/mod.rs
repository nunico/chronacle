//! PDF text extraction abstraction.
//!
//! Backed by `pdfium-render` (Chromium's PDF engine) for layout-aware
//! extraction that handles multi-column TTRPG rulebooks correctly. The
//! library is loaded at runtime from a bundled binary; see `build.rs`.
mod pdfium;
mod types;

pub use pdfium::PdfiumExtractor;
pub use types::{PageProgressFn, PdfExtractError, PdfExtractor};

#[cfg(test)]
mod test_builders;
#[cfg(test)]
mod tests;
