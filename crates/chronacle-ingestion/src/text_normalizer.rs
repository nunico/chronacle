//! Repairs PDF extraction artifacts before chunking.
//!
//! Three repairs in order:
//! 1. Soft-hyphen line joins: `power-\nful` → `powerful`.
//! 2. Single newlines inside paragraphs → space; double newlines preserved.
//! 3. Collapse runs of horizontal whitespace.
//!
//! Idempotent: `normalize(normalize(x)) == normalize(x)`.

pub fn normalize(text: &str) -> String {
    if text.trim().is_empty() {
        return String::new();
    }

    // Step 1: rejoin soft-hyphenated line breaks ("-\n" → "")
    // Only when the char before '-' is alphabetic and the char after '\n' is
    // alphabetic — avoids mangling intentional em-dash-style breaks.
    let chars: Vec<char> = text.chars().collect();
    let mut step1 = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '-'
            && i + 1 < chars.len()
            && chars[i + 1] == '\n'
            && i > 0
            && chars[i - 1].is_alphabetic()
            && i + 2 < chars.len()
            && chars[i + 2].is_alphabetic()
        {
            i += 2;
            continue;
        }
        step1.push(chars[i]);
        i += 1;
    }

    // Step 2: collapse single newlines to spaces; preserve "\n\n" boundaries.
    let chars: Vec<char> = step1.chars().collect();
    let mut step2 = String::with_capacity(step1.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\n' {
            let start = i;
            while i < chars.len() && chars[i] == '\n' {
                i += 1;
            }
            if i - start >= 2 {
                step2.push_str("\n\n");
            } else {
                step2.push(' ');
            }
            continue;
        }
        step2.push(chars[i]);
        i += 1;
    }

    // Step 3: collapse runs of horizontal whitespace within lines.
    let mut out = String::with_capacity(step2.len());
    let mut last_was_space = false;
    for c in step2.chars() {
        if c == '\n' {
            // Strip trailing space before a newline
            while out.ends_with(' ') {
                out.pop();
            }
            out.push(c);
            last_was_space = false;
            continue;
        }
        if c.is_whitespace() {
            if !last_was_space {
                out.push(' ');
            }
            last_was_space = true;
        } else {
            out.push(c);
            last_was_space = false;
        }
    }

    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_soft_hyphenated_word_at_line_break() {
        let input = "the union of\nfree traders; the mercenaries of the\nLegion";
        let output = normalize(input);
        assert!(output.contains("union of free traders"));
        assert!(output.contains("the mercenaries of the Legion"));
    }

    #[test]
    fn removes_hyphen_at_end_of_line() {
        let input = "power-\nful";
        assert_eq!(normalize(input), "powerful");
    }

    #[test]
    fn removes_multiple_hyphenated_breaks() {
        let input = "descen-\ndents of the cap-\ntain family";
        assert_eq!(normalize(input), "descendents of the captain family");
    }

    #[test]
    fn preserves_paragraph_breaks() {
        let input = "First paragraph.\n\nSecond paragraph.";
        let out = normalize(input);
        assert!(out.contains("First paragraph."));
        assert!(out.contains("Second paragraph."));
        assert!(
            out.contains("\n\n"),
            "paragraph break must survive: {out:?}"
        );
    }

    #[test]
    fn collapses_runs_of_spaces() {
        assert_eq!(normalize("a    b\t\tc"), "a b c");
    }

    #[test]
    fn is_idempotent() {
        let input = "power-\nful descen-\ndents\n\nNew para.";
        let once = normalize(input);
        let twice = normalize(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn does_not_join_intentional_hyphenated_compound_at_word_boundary() {
        let input = "state-of-the-art system";
        assert_eq!(normalize(input), "state-of-the-art system");
    }

    #[test]
    fn handles_empty_string() {
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn handles_only_whitespace() {
        assert_eq!(normalize("   \n\n  \t  "), "");
    }
}
