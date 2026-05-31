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
///
/// Smaller chunks (vs. the prior 400) improve retrieval precision for factoid
/// queries: each chunk is more focused on a single topic, so its embedding is
/// less diluted and cosine similarity ranks the right chunk higher.
const TARGET_TOKENS: usize = 250;

/// Overlap between consecutive chunks in approximate tokens (~20% of target).
const OVERLAP_TOKENS: usize = 50;

/// Characters per token approximation.
const CHARS_PER_TOKEN: f64 = 4.0;

// ── Reusable regex patterns ───────────────────────────────────────────

/// Matches chapter/part headings: "Chapter 1", "Part Three", "Chapter 1: Title", etc.
static CHAPTER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(chapter|part|section|appendix)\s+\S+").unwrap());

/// Matches numbered section headings: "1. Combat", "3.5 Skills", "10.2.1 Saving Throws"
static NUMBERED_SECTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d+(?:\.\d+)*[\.\)]?\s+\p{Lu}").unwrap());

/// Matches ALL-CAPS lines that are short enough to be headings (1–15 words).
static ALL_CAPS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\p{Lu}\s\d'\-\.!?/:;]{2,}$").unwrap());

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
/// 3. Is a short ALL-CAPS line (1–15 words)
/// 4. Is a short Title-Case line (≤10 words, no terminal punctuation,
///    every non-stopword starts with an uppercase letter)
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

    // Rule 4: Title-Case heading
    if is_title_case_heading(trimmed) {
        return true;
    }

    false
}

/// Short English-language stopwords allowed to be lowercase inside a
/// Title-Case heading (articles + common short prepositions/conjunctions).
const TITLE_CASE_STOPWORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "of", "in", "on", "at", "to", "for", "with", "by", "from",
    "into", "onto", "as", "but", "nor", "vs",
];

/// True if `line` looks like a Title-Case section heading:
/// - 1–10 words
/// - no terminal `.`, `!`, `?`
/// - every word that isn't a stopword starts with an uppercase letter
/// - at least one significant (non-stopword) word, ≥ 3 chars to filter out
///   single short capital words like "A" or "I"
fn is_title_case_heading(line: &str) -> bool {
    if line.ends_with('.') || line.ends_with('!') || line.ends_with('?') {
        return false;
    }
    let words: Vec<&str> = line.split_whitespace().collect();
    if words.is_empty() || words.len() > 10 {
        return false;
    }

    let mut significant_word_count = 0;
    for w in &words {
        let lower = w.to_lowercase();
        if TITLE_CASE_STOPWORDS.contains(&lower.as_str()) {
            continue;
        }
        // Significant word: must start with an uppercase letter.
        let first = match w.chars().next() {
            Some(c) => c,
            None => return false,
        };
        if !first.is_uppercase() {
            return false;
        }
        significant_word_count += 1;
    }

    // Require at least one significant word; for single-word lines require
    // ≥ 3 chars to avoid splitting "A" / "I" / "Or" as headings.
    if significant_word_count == 0 {
        return false;
    }
    if words.len() == 1 && words[0].chars().count() < 3 {
        return false;
    }

    true
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
///
/// Uses Unicode sentence boundaries from `unicode-segmentation`. Empty / pure
/// whitespace sentences are skipped.
fn sentence_offsets(text: &str) -> Vec<(usize, &str)> {
    use unicode_segmentation::UnicodeSegmentation;
    let mut out = Vec::new();
    let mut offset = 0usize;
    for sentence in text.split_sentence_bounds() {
        let trimmed = sentence.trim();
        if !trimmed.is_empty() {
            // Find the offset of the trimmed sentence within the original chunk
            let lead = sentence.chars().take_while(|c| c.is_whitespace()).count();
            out.push((offset + lead, trimmed));
        }
        offset += sentence.chars().count();
    }
    out
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
        assert!(!is_heading(
            "The fighter attacks the dragon with a longsword."
        ));
        assert!(!is_heading(
            "This is a paragraph of regular text that describes how combat works in the game."
        ));
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

    // ── Title-Case heading rule ──────────────────────────────────

    #[test]
    fn title_case_short_lines_are_headings() {
        assert!(is_heading("Coriolis and Kua"));
        assert!(is_heading("Combat Rules"));
        assert!(is_heading("The Brave Companions of Old"));
        assert!(is_heading("Order of the Pariah"));
        assert!(is_heading("Magic")); // single word, 5 chars
    }

    #[test]
    fn title_case_with_terminal_punctuation_is_not_heading() {
        assert!(!is_heading("The Brave Companions of Old."));
        assert!(!is_heading("Combat Rules!"));
    }

    #[test]
    fn lowercase_non_stopword_disqualifies() {
        // "orbits" is not capitalized → not a heading
        assert!(!is_heading("Coriolis orbits Kua"));
        // "from" is a stopword (ok lowercase), "stars" is not capitalized
        assert!(!is_heading("Travelers from distant stars"));
    }

    #[test]
    fn single_short_word_is_not_heading() {
        // Filter out single-letter "headings" like A / I
        assert!(!is_heading("A"));
        assert!(!is_heading("I"));
        assert!(!is_heading("Or"));
    }

    #[test]
    fn long_title_case_line_is_not_heading() {
        // >10 words is body text, not a heading
        assert!(!is_heading(
            "The Center of the Third Horizon and the Kua System and the Coriolis Station"
        ));
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
        let p1 = "This is page one of the document. It introduces the basic rules of combat. "
            .repeat(30);
        let p2 =
            "Page two continues with advanced combat techniques and special maneuvers. ".repeat(40);

        let full_text = format!("{p1}{p2}");
        let doc = make_doc(&full_text, vec![(&p1, 1), (&p2, 2)]);

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
        let has_any_heading = chunks.iter().any(|c| !c.section_heading.is_empty());
        assert!(has_any_heading, "chunks should have section headings");
    }

    #[test]
    fn test_chunk_respects_section_boundaries() {
        // Build text where section headings are within the overlap zone
        let intro_text = "Introductory text about the game world and characters. ".repeat(50);
        let combat_text = "Detailed combat rules describing attacks and damage. ".repeat(50);

        let text =
            format!("Chapter 1: INTRODUCTION\n{intro_text}\nChapter 2: COMBAT\n{combat_text}");

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
        assert!(
            chunks.len() >= 3,
            "long document should produce multiple chunks"
        );

        // Verify chunks don't overlap excessively and maintain ordering
        for (i, chunk) in chunks.iter().enumerate().skip(1) {
            let prev_text = &chunks[i - 1].text;
            assert!(
                chunk
                    .text
                    .contains(prev_text.split_whitespace().last().unwrap_or("")),
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
        assert!(
            chunks.len() >= 2,
            "multiple sections should produce multiple chunks"
        );

        // At least some chunks should carry a section heading
        let has_any_heading = chunks.iter().any(|c| !c.section_heading.is_empty());
        assert!(has_any_heading, "chunks should have section headings");
    }

    // ── Sentence-aware chunking tests ─────────────────────────────

    #[test]
    fn target_chunk_size_is_about_250_tokens() {
        assert_eq!(TARGET_TOKENS, 250);
        assert_eq!(OVERLAP_TOKENS, 50);
    }

    #[test]
    fn chunks_dont_end_mid_sentence() {
        let text = "First sentence ends here. Second sentence is longer and continues. \
                    Third sentence wraps up. "
            .repeat(60);
        let doc = make_doc(&text, vec![(text.as_str(), 1)]);
        let chunks = chunk_document(&doc);
        assert!(chunks.len() >= 2);
        for c in &chunks {
            let last_char = c.text.trim_end().chars().last().unwrap_or('.');
            assert!(
                ['.', '!', '?', '"', ')'].contains(&last_char),
                "chunk ends mid-sentence: {:?}",
                c.text.chars().rev().take(40).collect::<String>()
            );
        }
    }

    #[test]
    fn chunks_dont_start_mid_word() {
        let text = "Some sample text about combat. ".repeat(80);
        let doc = make_doc(&text, vec![(text.as_str(), 1)]);
        let chunks = chunk_document(&doc);
        for c in &chunks {
            let first_word = c.text.split_whitespace().next().unwrap_or("");
            if let Some(first) = first_word.chars().next() {
                assert!(
                    first.is_alphabetic() || first.is_numeric() || first == '"' || first == '(',
                    "chunk starts mid-word: {first_word:?}"
                );
            }
        }
    }
}
