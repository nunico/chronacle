//! Vault file body grammar.
//!
//! The compiled article lives inside an HTML-comment fence. Everything outside
//! the fence and outside a leading `## Summary` is GM-owned `notes`, verbatim.
//! The grammar is **lossless by construction**: outbound renders from the
//! record, so any unrecognised prose would otherwise be destroyed by the next
//! compile the GM never asked for.

/// Opening marker of the compiler-owned region.
pub const FENCE_START: &str =
    "<!-- chronacle:codex-article start -- compiled; edits are not applied -->";
/// Closing marker of the compiler-owned region.
pub const FENCE_END: &str = "<!-- chronacle:codex-article end -->";
/// Heading that delimits the GM-owned `summary` field.
pub const SUMMARY_HEADING: &str = "## Summary";
/// Heading emitted above `notes`. A rendering convention only — its absence
/// on parse is fine; `## Notes` is not required to classify text as notes.
pub const NOTES_HEADING: &str = "## Notes";

/// The three regions of a vault file body.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BodyParts {
    /// GM-owned. `None` for sessions and rule entries.
    pub summary: Option<String>,
    /// Compiler-owned: `codex_article` or `rule_entry.body`. Never applied inbound.
    pub fenced: Option<String>,
    /// GM-owned. Everything else, verbatim.
    pub notes: Option<String>,
}

/// Trim and normalise line endings. **Every** comparison in the engine runs on
/// normalized text — a byte-exact compare would manufacture a conflict each
/// time an editor appends a trailing newline.
pub fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n").trim().to_owned()
}

/// Wrap a normalized string as `Some`, or `None` if it is empty.
fn some_if_nonempty(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Split a vault file body into its three regions.
///
/// See the module doc for why this must be lossless: everything the parser
/// does not recognise ends up in `notes`, verbatim.
pub fn split_body(body: &str) -> BodyParts {
    // Step 1: cut the fence region, but only if both markers are present in
    // order. A dangling FENCE_START with no FENCE_END leaves the text in
    // place — the GM's prose survives instead of being silently reclassified.
    let (remainder, fenced) = match body.find(FENCE_START) {
        Some(start_idx) => {
            let after_start = start_idx + FENCE_START.len();
            match body[after_start..].find(FENCE_END) {
                Some(rel_end_idx) => {
                    let end_idx = after_start + rel_end_idx;
                    let fence_content = &body[after_start..end_idx];
                    let after_end = end_idx + FENCE_END.len();
                    let mut rest = String::with_capacity(body.len());
                    rest.push_str(&body[..start_idx]);
                    rest.push_str(&body[after_end..]);
                    (rest, Some(normalize(fence_content)))
                }
                None => (body.to_owned(), None),
            }
        }
        None => (body.to_owned(), None),
    };

    // Step 2: extract a leading `## Summary` section, if the first non-blank
    // line of the remainder is exactly SUMMARY_HEADING.
    let (after_summary, summary) = extract_leading_summary(&remainder);

    // Step 3: drop a leading NOTES_HEADING line, if present; the rest is notes.
    let notes_text = strip_leading_notes_heading(&after_summary);
    let notes = some_if_nonempty(normalize(&notes_text));

    BodyParts {
        summary,
        fenced,
        notes,
    }
}

/// If the first non-blank line of `text` is exactly `SUMMARY_HEADING`, return
/// `(remainder_with_summary_cut_out, Some(summary))`; otherwise
/// `(text unchanged, None)`.
fn extract_leading_summary(text: &str) -> (String, Option<String>) {
    // Find the start of the first non-blank line.
    let mut offset = 0usize;
    let mut found_heading = false;
    for line in text.split_inclusive('\n') {
        if line.trim().is_empty() {
            offset += line.len();
            continue;
        }
        if line.trim_end_matches('\n') != SUMMARY_HEADING {
            return (text.to_owned(), None);
        }
        found_heading = true;
        break;
    }
    // The text is empty or entirely blank lines: no heading was found, so
    // there is nothing to extract. Made explicit rather than relying on the
    // loop falling through with `offset` still valid.
    if !found_heading {
        return (text.to_owned(), None);
    }
    // `offset` now points at the start of the SUMMARY_HEADING line.
    let heading_line_len = text[offset..]
        .split_inclusive('\n')
        .next()
        .map(str::len)
        .unwrap_or(0);
    let after_heading = offset + heading_line_len;

    // Find the next `## ` heading at line start, scanning line-by-line.
    let mut search_from = after_heading;
    let mut section_end = text.len();
    loop {
        match text[search_from..].find('\n') {
            Some(rel_nl) => {
                let line_start = search_from;
                let line = &text[line_start..line_start + rel_nl];
                if line.starts_with("## ") {
                    section_end = line_start;
                    break;
                }
                search_from = line_start + rel_nl + 1;
            }
            None => {
                let line = &text[search_from..];
                if line.starts_with("## ") {
                    section_end = search_from;
                }
                break;
            }
        }
    }

    let summary_text = normalize(&text[after_heading..section_end]);
    let remainder = format!("{}{}", &text[..offset], &text[section_end..]);
    (remainder, some_if_nonempty(summary_text))
}

/// Drop a leading `## Notes` line, if the first non-blank line is exactly that.
fn strip_leading_notes_heading(text: &str) -> String {
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        if line.trim().is_empty() {
            offset += line.len();
            continue;
        }
        if line.trim_end_matches('\n') == NOTES_HEADING {
            let heading_len = line.len();
            return format!("{}{}", &text[..offset], &text[offset + heading_len..]);
        }
        break;
    }
    text.to_owned()
}

/// Render a vault file body from its parts. Round-trips with [`split_body`].
pub fn render_body(parts: &BodyParts) -> String {
    let mut sections: Vec<String> = Vec::new();
    if let Some(summary) = &parts.summary {
        sections.push(format!("{SUMMARY_HEADING}\n\n{summary}"));
    }
    if let Some(fenced) = &parts.fenced {
        sections.push(format!("{FENCE_START}\n{fenced}\n{FENCE_END}"));
    }
    if let Some(notes) = &parts.notes {
        sections.push(format!("{NOTES_HEADING}\n\n{notes}"));
    }
    if sections.is_empty() {
        return String::new();
    }
    format!("\n{}\n", sections.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn normalize_strips_crlf_and_trims() {
        assert_eq!(normalize("a\r\nb\r\n"), "a\nb");
        assert_eq!(normalize("  a\n\n"), "a");
        assert_eq!(normalize("a"), "a");
    }

    #[test]
    fn normalize_makes_a_trailing_newline_invisible() {
        // The whole point: an editor appending "\n" must not manufacture a conflict.
        assert_eq!(normalize("body"), normalize("body\n"));
        assert_eq!(normalize("body"), normalize("body\r\n"));
    }

    #[test]
    fn split_body_extracts_summary_fence_and_notes() {
        let body = format!(
            "\n## Summary\n\nA short summary.\n\n{FENCE_START}\nCompiled text.\n{FENCE_END}\n\n## Notes\n\nGM notes.\n"
        );
        let parts = split_body(&body);
        assert_eq!(parts.summary.as_deref(), Some("A short summary."));
        assert_eq!(parts.fenced.as_deref(), Some("Compiled text."));
        assert_eq!(parts.notes.as_deref(), Some("GM notes."));
    }

    #[test]
    fn split_body_keeps_unknown_headings_in_notes() {
        let body = format!(
            "\n## Summary\n\nS.\n\n{FENCE_START}\nC.\n{FENCE_END}\n\n## Notes\n\nN.\n\n## Ideas\n\nAn idea.\n"
        );
        let parts = split_body(&body);
        let notes = parts.notes.expect("notes");
        assert!(notes.contains("N."), "got {notes:?}");
        assert!(
            notes.contains("## Ideas"),
            "unknown heading must survive: {notes:?}"
        );
        assert!(
            notes.contains("An idea."),
            "unknown section body must survive: {notes:?}"
        );
    }

    #[test]
    fn split_body_keeps_prose_written_above_the_first_heading() {
        let body = "\nStray prose.\n\n## Notes\n\nN.\n";
        let parts = split_body(body);
        let notes = parts.notes.expect("notes");
        assert!(notes.contains("Stray prose."), "got {notes:?}");
    }

    #[test]
    fn split_body_treats_a_deleted_notes_heading_as_notes() {
        let body = format!("\n{FENCE_START}\nC.\n{FENCE_END}\n\nJust prose, no heading.\n");
        let parts = split_body(&body);
        assert_eq!(parts.fenced.as_deref(), Some("C."));
        assert_eq!(parts.notes.as_deref(), Some("Just prose, no heading."));
    }

    #[test]
    fn split_body_handles_a_session_file_with_no_fence() {
        let parts = split_body("\nSession recap.\n");
        assert_eq!(parts.fenced, None);
        assert_eq!(parts.summary, None);
        assert_eq!(parts.notes.as_deref(), Some("Session recap."));
    }

    #[test]
    fn render_body_then_split_body_round_trips() {
        let parts = BodyParts {
            summary: Some("S.".into()),
            fenced: Some("C.".into()),
            notes: Some("N.\n\n## Ideas\n\nAn idea.".into()),
        };
        let rendered = render_body(&parts);
        let back = split_body(&rendered);
        assert_eq!(back.summary, parts.summary);
        assert_eq!(back.fenced, parts.fenced);
        assert_eq!(back.notes, parts.notes);
    }

    #[test]
    fn render_body_omits_absent_sections() {
        let parts = BodyParts {
            summary: None,
            fenced: None,
            notes: Some("N.".into()),
        };
        let out = render_body(&parts);
        assert!(!out.contains(SUMMARY_HEADING));
        assert!(!out.contains(FENCE_START));
        assert!(out.contains("N."));
    }

    #[test]
    fn split_body_handles_an_empty_body() {
        let parts = split_body("");
        assert_eq!(parts.summary, None);
        assert_eq!(parts.fenced, None);
        assert_eq!(parts.notes, None);
    }

    #[test]
    fn split_body_handles_a_whitespace_only_body() {
        let parts = split_body("   \n\n  \n");
        assert_eq!(parts.summary, None);
        assert_eq!(parts.fenced, None);
        assert_eq!(parts.notes, None);
    }

    #[test]
    fn render_body_on_all_none_parts_is_empty() {
        assert_eq!(render_body(&BodyParts::default()), "");
    }

    #[test]
    fn an_unterminated_fence_is_treated_as_notes_not_as_article() {
        // A GM who deletes the closing marker must not have their prose
        // silently reclassified as compiler-owned.
        let body = format!("\n{FENCE_START}\nDangling.\n");
        let parts = split_body(&body);
        assert_eq!(parts.fenced, None, "no end marker => no fence");
        assert!(parts.notes.expect("notes").contains("Dangling."));
    }
}
