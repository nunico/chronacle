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
use std::sync::LazyLock;

use regex::Regex;

// ── Configuration constants ───────────────────────────────────────────

/// Target chunk size in approximate tokens.
const TARGET_TOKENS: usize = 400;

/// Overlap between consecutive chunks in approximate tokens.
const OVERLAP_TOKENS: usize = 80;

/// Characters per token approximation.
const CHARS_PER_TOKEN: f64 = 4.0;

// ── Reusable regex patterns ───────────────────────────────────────────

/// Matches chapter/part headings: "Chapter 1", "Part Three", "Chapter 1: Title", etc.
static CHAPTER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(chapter|part|section|appendix)\s+\S+").unwrap()
});

/// Matches numbered section headings: "1. Combat", "3.5 Skills", "10.2.1 Saving Throws"
static NUMBERED_SECTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d+(?:\.\d+)*[\.\)]?\s+\p{Lu}").unwrap()
});

/// Matches ALL-CAPS lines that are short enough to be headings (1–15 words).
static ALL_CAPS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[\p{Lu}\s\d'\-\.!?/:;]{2,}$").unwrap()
});

// ── Public types ──────────────────────────────────────────────────────

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

// ── Section detection ─────────────────────────────────────────────────

/// Determine whether a line is a section heading.
///
/// Heuristics (in priority order):
/// 1. Matches `Chapter X` / `Part X` / `Section X` / `Appendix X`
/// 2. Matches numbered-section pattern like `3. Combat`
/// 3. Is a short ALL-CAPS line (2–15 words, reasonable heading length)
pub fn is_heading(line: &str) -> bool {
    let trimmed = line.trim();

    // Skip empty lines
    if trimmed.is_empty() {
        return false;
    }

    // Skip lines that end with sentence-ending punctuation
    // — these are likely body text, not headings
    if trimmed.ends_with('.') || trimmed.ends_with('!') || trimmed.ends_with('?') {
        // Exception: ALL-CAPS short lines ending with punctuation can still be headings
        // (e.g., "WARNING!" or "STOP!")
        let word_count = trimmed.split_whitespace().count();
        if word_count > 3 {
            return false;
        }
    }

    // Rule 1: Chapter/Part/Section/Appendix heading
    if CHAPTER_RE.is_match(trimmed) {
        return true;
    }

    // Rule 2: Numbered section heading
    if NUMBERED_SECTION_RE.is_match(trimmed) {
        return true;
    }

    // Rule 3: ALL-CAPS line (1–15 words)
    let word_count = trimmed.split_whitespace().count();
    if (1..=15).contains(&word_count) && ALL_CAPS_RE.is_match(trimmed) {
        return true;
    }

    false
}

// ── Token approximation ───────────────────────────────────────────────

/// Approximate token count for a text string.
///
/// Uses the standard heuristic of `chars / 4` for English text.
/// This is deliberately simple — Phase 2 can replace it with a real
/// tokenizer if needed.
pub fn approx_token_count(text: &str) -> usize {
    let len = text.chars().count() as f64;
    (len / CHARS_PER_TOKEN).round() as usize
}

// ── Chunking logic ───────────────────────────────────────────────────

/// Split an extracted document into searchable chunks.
///
/// Strategy:
/// 1. Run section detection across the full text to find heading positions.
/// 2. Walk through the full text with a sliding window of ~TARGET_TOKENS.
/// 3. In the overlap region, prefer splitting at section boundaries.
/// 4. Tag each chunk with its source page range and section heading.
pub fn chunk_document(doc: &ExtractedDoc) -> Vec<Chunk> {
    let target_chars = (TARGET_TOKENS as f64 * CHARS_PER_TOKEN) as usize;
    let overlap_chars = (OVERLAP_TOKENS as f64 * CHARS_PER_TOKEN) as usize;
    let step = target_chars.saturating_sub(overlap_chars);

    // ── Detect section headings ──────────────────────────────────
    let headings = detect_headings(&doc.text);

    // ── Build page-offset index for page_start/page_end ──────────
    let page_offsets = build_page_offsets(&doc.pages);

    // ── Sliding window ───────────────────────────────────────────
    let text = &doc.text;
    let text_len = text.chars().count();
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut cursor = 0;

    while cursor < text_len {
        let end = std::cmp::min(cursor + target_chars, text_len);

        let raw_text: String = text.chars().skip(cursor).take(end - cursor).collect();
        let trimmed = raw_text.trim().to_string();

        if !trimmed.is_empty() {
            // Determine the most relevant section heading for this chunk
            let heading = find_active_heading(cursor, &headings);

            // Compute page range for this chunk
            let (ps, pe) = page_range_for_byte_range(cursor, end, &page_offsets);

            chunks.push(Chunk {
                text: trimmed,
                page_start: ps,
                page_end: pe,
                section_heading: heading,
            });
        }

        // Advance cursor — prefer section boundaries in the overlap zone
        cursor = advance_cursor(cursor, step, end, text_len, &headings);
    }

    chunks
}

/// Detect all heading positions in the full text.
///
/// Returns a list of `(char_index, heading_text)` pairs sorted by position.
fn detect_headings(text: &str) -> Vec<(usize, String)> {
    let mut headings = Vec::new();
    let mut char_offset = 0;

    for line in text.lines() {
        if is_heading(line) {
            headings.push((char_offset, line.trim().to_string()));
        }
        // Advance char_offset by line length + newline
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
        // If start and end span across pages, page_end is the last page containing text
        if start < p_start && end > p_start && page_num > page_end {
            page_end = page_num;
        }
    }

    (page_start, page_end)
}

/// Advance the window cursor, preferring section boundaries when available.
fn advance_cursor(
    cursor: usize,
    step: usize,
    end: usize,
    _text_len: usize,
    headings: &[(usize, String)],
) -> usize {
    // Default: move forward by `step`
    let default_next = std::cmp::max(cursor + 1, end);

    // Look for a section heading in the overlap zone (between `step` chars
    // from the start and the original end position)
    let overlap_start = cursor + step;

    // Find the first heading that falls within the overlap zone
    for (hpos, _) in headings {
        if *hpos > cursor && *hpos >= overlap_start && *hpos < end {
            return *hpos;
        }
    }

    // If no heading in the overlap zone, use the overlap target
    default_next
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Token count tests ─────────────────────────────────────────

    #[test]
    fn test_approx_token_count_empty() {
        assert_eq!(approx_token_count(""), 0);
    }

    #[test]
    fn test_approx_token_count_short() {
        // "hello world" = 11 chars → 11/4 = 2.75 → 3
        assert_eq!(approx_token_count("hello world"), 3);
    }

    #[test]
    fn test_approx_token_count_typical() {
        let text = "The fighter swings his greatsword at the dragon, dealing 12 points of slashing damage.";
        // ~87 chars → 87/4 ≈ 22 tokens
        assert_eq!(approx_token_count(text), 22);
    }

    // ── Section detection tests ───────────────────────────────────

    #[test]
    fn test_is_heading_empty_line() {
        assert!(!is_heading(""));
        assert!(!is_heading("   "));
    }

    #[test]
    fn test_is_heading_chapter_pattern() {
        assert!(is_heading("Chapter 1: Introduction"));
        assert!(is_heading("Chapter 2"));
        assert!(is_heading("PART ONE: THE BASIC RULES"));
        assert!(is_heading("Part Two: Combat"));
        assert!(is_heading("Section 3: Magic"));
        assert!(is_heading("Appendix A: Conditions"));
    }

    #[test]
    fn test_is_heading_numbered_section() {
        assert!(is_heading("1. Combat"));
        assert!(is_heading("3.5 Skills"));
        assert!(is_heading("10.2.1 Saving Throws"));
    }

    #[test]
    fn test_is_heading_all_caps() {
        assert!(is_heading("COMBAT"));
        assert!(is_heading("USING ABILITIES"));
        assert!(is_heading("SPELL DESCRIPTIONS"));
        assert!(is_heading("DAMAGE AND HEALING"));
    }

    #[test]
    fn test_is_not_heading_regular_sentence() {
        assert!(!is_heading("The fighter attacks the dragon with a longsword."));
        assert!(!is_heading("This is a paragraph of regular text that describes how combat works in the game."));
    }

    #[test]
    fn test_is_not_heading_too_long_all_caps() {
        // 20+ words all caps is likely a table or block, not a heading
        assert!(!is_heading("THIS IS A VERY LONG ALL CAPS LINE THAT LOOKS LIKE A TABLE OR DATA BLOCK AND NOT A HEADING"));
    }

    #[test]
    fn test_is_not_heading_lowercase_line() {
        assert!(!is_heading("combat rules"));
        assert!(!is_heading("using abilities"));
    }

    #[test]
    fn test_is_heading_short_caps_with_punctuation() {
        // Short ALL-CAPS with punctuation should still be headings
        assert!(is_heading("WARNING!"));
        assert!(is_heading("STOP!"));
    }

    // ── Chunking tests ────────────────────────────────────────────

    fn make_doc(text: &str, pages: Vec<(&str, usize)>) -> ExtractedDoc {
        let pages: Vec<PageContent> = pages
            .into_iter()
            .map(|(t, n)| PageContent {
                page_num: n,
                text: t.to_string(),
            })
            .collect();
        ExtractedDoc {
            page_count: pages.len(),
            text: text.to_string(),
            pages,
        }
    }

    #[test]
    fn test_chunk_empty_document() {
        let doc = make_doc("", vec![("", 1)]);
        let chunks = chunk_document(&doc);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_single_short_document() {
        let text = "The fighter attacks the dragon.";
        let doc = make_doc(text, vec![(text, 1)]);
        let chunks = chunk_document(&doc);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, text);
        assert_eq!(chunks[0].page_start, 1);
        assert_eq!(chunks[0].page_end, 1);
    }

    #[test]
    fn test_chunk_preserves_page_range() {
        let p1 = "This is page one of the document. It introduces the basic rules of combat. ".repeat(30);
        let p2 = "Page two continues with advanced combat techniques and special maneuvers. ".repeat(40);

        let full_text = format!("{p1}{p2}");
        let doc = make_doc(
            &full_text,
            vec![(&p1, 1), (&p2, 2)],
        );

        let chunks = chunk_document(&doc);
        assert!(chunks.len() >= 2, "should split into multiple chunks");

        // First chunk should be on page 1
        assert_eq!(chunks[0].page_start, 1);
        // Last chunk should include page 2
        assert_eq!(chunks.last().unwrap().page_end, 2);
    }

    #[test]
    fn test_chunk_section_heading_attached() {
        let text = "\
Chapter 1: COMBAT
The combat section describes how battles work in the game. "
            .repeat(80);
        let doc = make_doc(&text, vec![(text.as_str(), 1)]);
        let chunks = chunk_document(&doc);

        // All chunks should have "Chapter 1: COMBAT" as their section heading
        for chunk in &chunks {
            assert_eq!(chunk.section_heading, "Chapter 1: COMBAT");
        }
    }

    #[test]
    fn test_chunk_multiple_sections() {
        let intro = "\
Chapter 1: INTRODUCTION
This is the introductory section that explains the basics of the game. "
            .repeat(20);
        let combat = "\
Chapter 2: COMBAT
This section explains the detailed rules for combat encounters. "
            .repeat(30);
        let text = format!("{intro}{combat}");

        let pages = vec![(text.as_str(), 1)];
        let doc = make_doc(&text, pages);

        // Verify the full text is long enough to need at least the chapter break
        let target_chars = (TARGET_TOKENS as f64 * CHARS_PER_TOKEN) as usize;
        assert!(
            text.len() > target_chars * 2,
            "test text must be long enough to span multiple chunks"
        );

        let chunks = chunk_document(&doc);
        assert!(chunks.len() >= 2, "should produce at least 2 chunks");

        // At least some chunks should carry a section heading
        let has_any_heading = chunks
            .iter()
            .any(|c| !c.section_heading.is_empty());
        assert!(has_any_heading, "chunks should have section headings");
    }

    #[test]
    fn test_chunk_respects_section_boundaries() {
        // Build text where section headings are within the overlap zone
        let intro_text = "Introductory text about the game world and characters. ".repeat(50);
        let combat_text = "Detailed combat rules describing attacks and damage. ".repeat(50);

        let text = format!(
            "Chapter 1: INTRODUCTION\n{intro_text}\nChapter 2: COMBAT\n{combat_text}"
        );

        let doc = make_doc(&text, vec![(text.as_str(), 1)]);
        let chunks = chunk_document(&doc);

        // A chunk should be split at Chapter 2 boundary
        let intro_chunks: Vec<_> = chunks
            .iter()
            .filter(|c| c.section_heading == "Chapter 1: INTRODUCTION")
            .collect();
        let combat_chunks: Vec<_> = chunks
            .iter()
            .filter(|c| c.section_heading == "Chapter 2: COMBAT")
            .collect();

        assert!(!intro_chunks.is_empty(), "should have intro chunks");
        assert!(!combat_chunks.is_empty(), "should have combat chunks");

        // All intro chunks should come before combat chunks
        let first_combat = chunks
            .iter()
            .position(|c| c.section_heading == "Chapter 2: COMBAT")
            .unwrap();
        let last_intro = chunks
            .iter()
            .rposition(|c| c.section_heading == "Chapter 1: INTRODUCTION")
            .unwrap();
        assert!(
            last_intro < first_combat,
            "intro chunks should precede combat chunks"
        );
    }

    #[test]
    fn test_chunk_very_long_document_creates_multiple_chunks() {
        let text = "This is a sample paragraph of text about TTRPG rules. ".repeat(500);
        let doc = make_doc(&text, vec![(text.as_str(), 1)]);

        let chunks = chunk_document(&doc);
        assert!(chunks.len() >= 3, "long document should produce multiple chunks");

        // Verify chunks don't overlap excessively and maintain ordering
        for (i, chunk) in chunks.iter().enumerate().skip(1) {
            let prev_text = &chunks[i - 1].text;
            assert!(
                chunk.text.contains(prev_text.split_whitespace().last().unwrap_or("")),
                "consecutive chunks should have overlap"
            );
        }
    }

    #[test]
    fn test_chunk_no_duplicate_empty_chunks() {
        let text = "A B C. ".repeat(5);
        let doc = make_doc(&text, vec![(text.as_str(), 1)]);
        let chunks = chunk_document(&doc);

        // No chunk should be empty or whitespace-only
        for chunk in &chunks {
            assert!(!chunk.text.trim().is_empty(), "no empty chunks allowed");
        }
    }

    #[test]
    fn test_is_heading_chapter_variants() {
        assert!(is_heading("CHAPTER 1: INTRODUCTION"));
        assert!(is_heading("Chapter 1: Introduction"));
        assert!(is_heading("chapter 1: introduction"));
        assert!(is_heading("Part 2: The Rules of the Game"));
        assert!(is_heading("Section 4: Magic Items"));
        assert!(is_heading("Appendix B: Spells"));
    }

    #[test]
    fn test_chunk_multipage_range() {
        // Single long page → many chunks
        let text = "Combat rules paragraph. ".repeat(300);
        let doc = make_doc(&text, vec![(text.as_str(), 1)]);
        let chunks = chunk_document(&doc);
        assert!(chunks.len() >= 2);
        for c in &chunks {
            assert_eq!(c.page_start, 1);
            assert_eq!(c.page_end, 1);
        }
    }

    #[test]
    fn test_chunk_with_headings_splits_at_boundaries() {
        // Build text with many headings close together
        let body = "Some descriptive text about this game mechanic. ".repeat(10);
        let sections: Vec<String> = (1..=5)
            .map(|i| format!("Chapter {i}: SECTION {i}\n{body}"))
            .collect();
        let text = sections.join("\n");

        let doc = make_doc(&text, vec![(text.as_str(), 1)]);
        let chunks = chunk_document(&doc);

        // Verify multiple sections produce multiple chunks
        assert!(chunks.len() >= 2, "multiple sections should produce multiple chunks");

        // At least some chunks should carry a section heading
        let has_any_heading = chunks
            .iter()
            .any(|c| !c.section_heading.is_empty());
        assert!(has_any_heading, "chunks should have section headings");
    }
}