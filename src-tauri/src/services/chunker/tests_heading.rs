use super::core::approx_token_count;
use super::heading::is_heading;

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
    assert!(is_heading("Lantern and Mirovia"));
    assert!(is_heading("Combat Rules"));
    assert!(is_heading("The Brave Companions of Old"));
    assert!(is_heading("Vows of the Lumen Order"));
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
    assert!(!is_heading("Lantern orbits Mirovia"));
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
        "The Center of the Ember Reach and the Velmar System and the Lantern Station"
    ));
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
