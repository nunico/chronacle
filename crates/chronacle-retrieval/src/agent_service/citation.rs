//! Citation extraction from assistant responses.

/// A single citation extracted from an assistant response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Citation {
    pub source_name: String,
    pub page: Option<i64>,
    pub text_excerpt: String,
}

/// Parse citations from an assistant response.
///
/// Accepts:
///   `[Source: "Name", p.12]`                        – page only
///   `[Source: "Name", p.45-49]`                     – page range (start captured)
///   `[Source: "Name", p.9, quote: "verbatim text"]` – with inline supporting quote
///   `[Source: "Name"]`                              – source only
///
/// When a quote is present, it's stored as `text_excerpt`. When absent, the
/// 80 characters following the citation marker are used as a degraded fallback.
pub(super) fn parse_citations(response: &str) -> Vec<Citation> {
    // Tolerant of model format drift: singular `quote:` or plural `quotes:`, and
    // any trailing content (a second excerpt, stray prose) up to the closing `]`
    // is consumed so the marker still parses. First quoted excerpt is captured.
    let re = regex::Regex::new(
        r#"(?s)\[Source:\s*"([^"]+)"(?:,\s*p\.\s*(\d+)(?:-\d+)?)?(?:,\s*quotes?:\s*"(.*?)")?[^\]]*\]"#,
    )
    .expect("valid citation regex");

    re.captures_iter(response)
        .map(|cap| {
            let source_name = cap[1].to_string();
            let page = cap.get(2).and_then(|m| m.as_str().parse::<i64>().ok());
            let text_excerpt = if let Some(q) = cap.get(3) {
                q.as_str().trim().to_string()
            } else {
                let marker_end = cap.get(0).map_or(0, |m| m.end());
                response
                    .chars()
                    .skip(marker_end)
                    .take(80)
                    .collect::<String>()
                    .trim()
                    .to_string()
            };

            Citation {
                source_name,
                page,
                text_excerpt,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_citations_empty() {
        let citations = parse_citations("Hello, I don't know the answer.");
        assert!(citations.is_empty());
    }

    #[test]
    fn test_parse_citations_single() {
        let text = "The fighter can use Action Surge [Source: \"PHB\", p.72].";
        let citations = parse_citations(text);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].source_name, "PHB");
        assert_eq!(citations[0].page, Some(72));
    }

    #[test]
    fn test_parse_citations_multiple() {
        let text = "Combat has multiple actions [Source: \"PHB\", p.192]. \
                     Opportunity attacks are different [Source: \"DMG\", p.25].";
        let citations = parse_citations(text);
        assert_eq!(citations.len(), 2);
        assert_eq!(citations[0].source_name, "PHB");
        assert_eq!(citations[1].source_name, "DMG");
    }

    #[test]
    fn test_parse_citations_no_page() {
        let text = "See the basic rules [Source: \"SRD\"].";
        let citations = parse_citations(text);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].source_name, "SRD");
        assert_eq!(citations[0].page, None);
    }

    #[test]
    fn test_parse_citations_with_inline_quote() {
        let text = "Lantern orbits Mirovia. [Source: \"Quickstart.pdf\", p.9, quote: \"The space station Lantern orbits the silver clouds of the planet Mirovia.\"]";
        let citations = parse_citations(text);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].source_name, "Quickstart.pdf");
        assert_eq!(citations[0].page, Some(9));
        assert_eq!(
            citations[0].text_excerpt,
            "The space station Lantern orbits the silver clouds of the planet Mirovia."
        );
    }

    #[test]
    fn test_parse_citations_inline_quote_with_page_range() {
        let text = "[Source: \"PHB\", p.45-49, quote: \"Combat proceeds in rounds.\"]";
        let citations = parse_citations(text);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].page, Some(45));
        assert_eq!(citations[0].text_excerpt, "Combat proceeds in rounds.");
    }

    // Field regression: the model drifted to plural `quotes:` with two excerpts
    // joined by "and". The strict `quote:` + `]` anchor failed to match, so the
    // citation was dropped (and the raw marker leaked into the rendered reply).
    #[test]
    fn test_parse_citations_plural_quotes_with_multiple_excerpts() {
        let text = "[Source: \"Coriolis EN.pdf\", p.214-215, quotes: \"Secure dangerous artifacts for... the Draconites\" and \"Prevent the spread of dangerous bionics for... the Draconites\"]";
        let citations = parse_citations(text);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].source_name, "Coriolis EN.pdf");
        assert_eq!(citations[0].page, Some(214));
        // The first excerpt is captured as the supporting quote.
        assert_eq!(
            citations[0].text_excerpt,
            "Secure dangerous artifacts for... the Draconites"
        );
    }

    #[test]
    fn test_parse_citations_page_range() {
        let cases = [
            (
                "[Source: \"Quickstart.pdf\", p.9-9]",
                "Quickstart.pdf",
                Some(9),
            ),
            (
                "[Source: \"Quickstart.pdf\", p.45-49]",
                "Quickstart.pdf",
                Some(45),
            ),
            ("[Source: \"PHB\", p. 72-72]", "PHB", Some(72)),
        ];
        for (input, expected_name, expected_page) in cases {
            let citations = parse_citations(input);
            assert_eq!(citations.len(), 1, "no match for {input:?}");
            assert_eq!(citations[0].source_name, expected_name);
            assert_eq!(citations[0].page, expected_page);
        }
    }
}
