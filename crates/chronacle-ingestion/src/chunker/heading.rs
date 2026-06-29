use std::sync::LazyLock;

use regex::Regex;

/// Matches chapter/part headings: "Chapter 1", "Part Three", "Chapter 1: Title", etc.
static CHAPTER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(chapter|part|section|appendix)\s+\S+").unwrap());

/// Matches numbered section headings: "1. Combat", "3.5 Skills", "10.2.1 Saving Throws"
static NUMBERED_SECTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d+(?:\.\d+)*[\.\)]?\s+\p{Lu}").unwrap());

/// Matches ALL-CAPS lines that are short enough to be headings (1–15 words).
static ALL_CAPS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\p{Lu}\s\d'\-\.!?/:;]{2,}$").unwrap());

/// Short English-language stopwords allowed to be lowercase inside a
/// Title-Case heading (articles + common short prepositions/conjunctions).
const TITLE_CASE_STOPWORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "of", "in", "on", "at", "to", "for", "with", "by", "from",
    "into", "onto", "as", "but", "nor", "vs",
];

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

/// True if `line` looks like a Title-Case section heading:
/// - 1–10 words
/// - no terminal `.`, `!`, `?`
/// - every word that isn't a stopword starts with an uppercase letter
/// - at least one significant (non-stopword) word, ≥ 3 chars to filter out
///   single short capital words like "A" or "I"
pub fn is_title_case_heading(line: &str) -> bool {
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
        let first = match w.chars().next() {
            Some(c) => c,
            None => return false,
        };
        if !first.is_uppercase() {
            return false;
        }
        significant_word_count += 1;
    }

    if significant_word_count == 0 {
        return false;
    }
    if words.len() == 1 && words[0].chars().count() < 3 {
        return false;
    }

    true
}
