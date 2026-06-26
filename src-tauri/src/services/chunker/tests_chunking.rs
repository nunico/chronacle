use super::core::{chunk_document, OVERLAP_TOKENS, TARGET_TOKENS};
use super::types::{ExtractedDoc, PageContent};

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

    let target_chars = (TARGET_TOKENS as f64 * 4.0) as usize;
    assert!(
        text.len() > target_chars * 2,
        "test text must be long enough to span multiple chunks"
    );

    let chunks = chunk_document(&doc);
    assert!(chunks.len() >= 2, "should produce at least 2 chunks");

    let has_any_heading = chunks.iter().any(|c| !c.section_heading.is_empty());
    assert!(has_any_heading, "chunks should have section headings");
}

#[test]
fn test_chunk_respects_section_boundaries() {
    let intro_text = "Introductory text about the game world and characters. ".repeat(50);
    let combat_text = "Detailed combat rules describing attacks and damage. ".repeat(50);

    let text =
        format!("Chapter 1: INTRODUCTION\n{intro_text}\nChapter 2: COMBAT\n{combat_text}");

    let doc = make_doc(&text, vec![(text.as_str(), 1)]);
    let chunks = chunk_document(&doc);

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

    for chunk in &chunks {
        assert!(!chunk.text.trim().is_empty(), "no empty chunks allowed");
    }
}

#[test]
fn test_chunk_multipage_range() {
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
    let body = "Some descriptive text about this game mechanic. ".repeat(10);
    let sections: Vec<String> = (1..=5)
        .map(|i| format!("Chapter {i}: SECTION {i}\n{body}"))
        .collect();
    let text = sections.join("\n");

    let doc = make_doc(&text, vec![(text.as_str(), 1)]);
    let chunks = chunk_document(&doc);

    assert!(
        chunks.len() >= 2,
        "multiple sections should produce multiple chunks"
    );

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
