use super::heading::is_heading;
use super::types::{Chunk, ExtractedDoc, PageContent};

/// Target chunk size in approximate tokens.
///
/// Smaller chunks improve retrieval precision for factoid queries: each chunk
/// is more focused on a single topic, so its embedding is less diluted and
/// cosine similarity ranks the right chunk higher.
pub(super) const TARGET_TOKENS: usize = 250;

/// Overlap between consecutive chunks in approximate tokens (~20% of target).
pub(super) const OVERLAP_TOKENS: usize = 50;

/// Characters per token approximation.
const CHARS_PER_TOKEN: f64 = 4.0;

/// Approximate token count for a text string.
///
/// Uses the standard heuristic of `chars / 4` for English text.
pub fn approx_token_count(text: &str) -> usize {
    let len = text.chars().count() as f64;
    (len / CHARS_PER_TOKEN).round() as usize
}

/// Split an extracted document into searchable chunks.
///
/// Strategy:
/// 1. Run section detection across the full text to find heading positions.
/// 2. Split the text into sentences and group sentences greedily up to
///    ~TARGET_TOKENS per chunk.
/// 3. Overlap consecutive chunks by ~OVERLAP_TOKENS worth of trailing sentences.
/// 4. Tag each chunk with its source page range and section heading.
pub fn chunk_document(doc: &ExtractedDoc) -> Vec<Chunk> {
    let target_chars = (TARGET_TOKENS as f64 * CHARS_PER_TOKEN) as usize;
    let overlap_chars = (OVERLAP_TOKENS as f64 * CHARS_PER_TOKEN) as usize;

    let headings = detect_headings(&doc.text);
    let page_offsets = build_page_offsets(&doc.pages);
    let sentences = sentence_offsets(&doc.text);

    if sentences.is_empty() {
        return Vec::new();
    }

    let mut chunks: Vec<Chunk> = Vec::new();
    let mut i = 0;
    while i < sentences.len() {
        let chunk_start_char = sentences[i].0;
        let mut chunk_text = String::new();
        let mut j = i;

        while j < sentences.len() {
            let next_len = sentences[j].1.chars().count();
            if !chunk_text.is_empty() && chunk_text.chars().count() + next_len + 1 > target_chars {
                break;
            }
            if !chunk_text.is_empty() {
                chunk_text.push(' ');
            }
            chunk_text.push_str(sentences[j].1);
            j += 1;
        }

        if !chunk_text.trim().is_empty() {
            let chunk_end_char = chunk_start_char + chunk_text.chars().count();
            let heading = find_active_heading(chunk_start_char, &headings);
            let (ps, pe) =
                page_range_for_byte_range(chunk_start_char, chunk_end_char, &page_offsets);

            chunks.push(Chunk {
                text: chunk_text.trim().to_string(),
                page_start: ps,
                page_end: pe,
                section_heading: heading,
            });
        }

        if j >= sentences.len() {
            break;
        }

        // Advance i so the next chunk overlaps by ~OVERLAP_TOKENS worth of
        // trailing sentences. Always make progress (at least one sentence).
        let mut overlap_size = 0usize;
        let mut new_i = j;
        while new_i > i + 1 && overlap_size < overlap_chars {
            new_i -= 1;
            overlap_size += sentences[new_i].1.chars().count();
        }
        i = std::cmp::max(new_i, i + 1);
    }

    chunks
}

/// Split text into sentences and return each sentence's starting char offset.
fn sentence_offsets(text: &str) -> Vec<(usize, &str)> {
    use unicode_segmentation::UnicodeSegmentation;
    let mut out = Vec::new();
    let mut offset = 0usize;
    for sentence in text.split_sentence_bounds() {
        let trimmed = sentence.trim();
        if !trimmed.is_empty() {
            let lead = sentence.chars().take_while(|c| c.is_whitespace()).count();
            out.push((offset + lead, trimmed));
        }
        offset += sentence.chars().count();
    }
    out
}

/// Detect all heading positions in the full text.
fn detect_headings(text: &str) -> Vec<(usize, String)> {
    let mut headings = Vec::new();
    let mut char_offset = 0;

    for line in text.lines() {
        if is_heading(line) {
            headings.push((char_offset, line.trim().to_string()));
        }
        char_offset += line.chars().count() + 1; // +1 for \n
    }

    headings
}

/// Build a list of `(start_offset, end_offset, page_num)` for each page.
fn build_page_offsets(pages: &[PageContent]) -> Vec<(usize, usize, i64)> {
    let mut offsets = Vec::new();
    let mut offset = 0;

    for page in pages {
        let start = offset;
        let page_len = page.text.chars().count();
        offset += page_len;
        let end = offset;
        offsets.push((start, end, page.page_num as i64));
    }

    offsets
}

/// Find the active section heading at a given character position.
fn find_active_heading(pos: usize, headings: &[(usize, String)]) -> String {
    headings
        .iter()
        .rev()
        .find(|(hpos, _)| *hpos <= pos)
        .map(|(_, heading)| heading.clone())
        .unwrap_or_default()
}

/// Compute page start/end for a given character range.
fn page_range_for_byte_range(
    start: usize,
    end: usize,
    page_offsets: &[(usize, usize, i64)],
) -> (i64, i64) {
    let mut page_start = 1i64;
    let mut page_end = 1i64;

    for &(p_start, p_end, page_num) in page_offsets {
        if p_start <= start && start < p_end {
            page_start = page_num;
        }
        if p_start < end && end <= p_end {
            page_end = page_num;
        }
        if start < p_start && end > p_start && page_num > page_end {
            page_end = page_num;
        }
    }

    (page_start, page_end)
}
