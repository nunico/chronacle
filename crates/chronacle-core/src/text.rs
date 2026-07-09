//! Text normalisation shared by every layer that emits single-line scalars.

/// Normalise a value destined for a single-line context (a YAML frontmatter
/// scalar, an entity name, a title, an alias).
///
/// Control characters carry no meaning in these fields and can only corrupt
/// the on-disk format, so they are removed rather than escaped. Whitespace —
/// including newlines and tabs — is collapsed to single spaces: a newline in a
/// name is a paste artefact, not intent. Multi-line text belongs in the file
/// body, never in frontmatter.
///
/// Idempotent: `sanitize_scalar(sanitize_scalar(s)) == sanitize_scalar(s)`.
/// Non-ASCII letters and emoji pass through untouched.
pub fn sanitize_scalar(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_was_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else if c.is_control() {
            // Dropped: control characters other than whitespace carry no
            // semantic value in a single-line scalar.
        } else {
            out.push(c);
            last_was_space = false;
        }
    }
    out.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newline_collapses_to_space() {
        assert_eq!(sanitize_scalar("Foo\nBar"), "Foo Bar");
    }

    #[test]
    fn tab_collapses_to_space() {
        assert_eq!(sanitize_scalar("Tab\there"), "Tab here");
    }

    #[test]
    fn bell_is_dropped() {
        assert_eq!(sanitize_scalar("Bell\u{7}x"), "Bellx");
    }

    #[test]
    fn del_is_dropped() {
        assert_eq!(sanitize_scalar("\u{7f}del"), "del");
    }

    #[test]
    fn leading_and_trailing_whitespace_is_trimmed() {
        assert_eq!(sanitize_scalar("  padded  "), "padded");
    }

    #[test]
    fn punctuation_is_kept_and_runs_of_spaces_collapse() {
        assert_eq!(sanitize_scalar("a  --  b"), "a -- b");
    }

    #[test]
    fn non_ascii_letters_and_emoji_pass_through() {
        assert_eq!(sanitize_scalar("Séraphina 日本語 🗡"), "Séraphina 日本語 🗡");
    }

    #[test]
    fn sanitize_scalar_is_idempotent() {
        let cases = [
            "Foo\nBar",
            "Tab\there",
            "Bell\u{7}x",
            "\u{7f}del",
            "  padded  ",
            "a  --  b",
            "Séraphina 日本語 🗡",
        ];
        for case in cases {
            let once = sanitize_scalar(case);
            let twice = sanitize_scalar(&once);
            assert_eq!(once, twice, "not idempotent for {case:?}");
        }
    }
}
