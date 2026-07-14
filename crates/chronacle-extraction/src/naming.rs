//! Pure name matching: normalization + trigram similarity. No I/O, no DB.
//!
//! Shared by wikilink resolution and duplicate detection so the two can never
//! disagree about whether two names are "the same".
//!
//! English-centric by design (leading "the", trailing plural "s"): the corpus
//! is English TTRPG material. Rules can grow here without touching callers.

/// Ranked near-match.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub id: String,
    pub name: String,
    pub similarity: f64,
}

/// The result of a fuzzy lookup. `Ambiguous` is NOT a failure — it is a refusal
/// to guess, and it is what the "did you mean …?" suggestion is built from.
#[derive(Debug, Clone, PartialEq)]
pub enum MatchOutcome<'a> {
    None,
    Unique {
        id: &'a str,
        name: &'a str,
        score: f64,
    },
    Ambiguous(Vec<Candidate>),
}

/// Provisional. Tuned against real campaign data in Task 10 and recorded in
/// ADR-012 with the evidence. Prefer a MISSED match (degrades to a suggestion)
/// over a FALSE one (silently corrupts the graph).
pub const DEFAULT_THRESHOLD: f64 = 0.72;

/// Case-fold, strip a leading "the", drop possessives, singularize a trailing
/// plural, collapse punctuation and whitespace. Never used for storage — only
/// ever as a lookup key. The GM's exact spelling is preserved as name or alias.
pub fn normalize(name: &str) -> String {
    let lowered = name.to_lowercase();
    let cleaned: String = lowered
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect();

    let words: Vec<&str> = cleaned.split_whitespace().collect();
    let words = match words.split_first() {
        Some((&"the", rest)) if !rest.is_empty() => rest.to_vec(),
        _ => words,
    };

    words
        .iter()
        .filter(|w| w.len() > 1 || **w == "a")
        .map(|w| singularize(w))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Conservative: a small rule set, not a stemmer. Over-eager singularization
/// merges distinct names, which is the expensive direction to be wrong in.
fn singularize(word: &str) -> String {
    // Possessives were already stripped to a bare "s" by punctuation removal
    // ("seraphina's" -> "seraphina s"), so a lone "s" is dropped by the
    // whitespace collapse and never reaches here.
    if word.len() > 3 && word.ends_with("ies") {
        return format!("{}y", &word[..word.len() - 3]);
    }
    if word.len() > 3 && word.ends_with("es") && !word.ends_with("ses") {
        return word[..word.len() - 2].to_string();
    }
    // "chaos", "ss" endings, "os" endings, and short words keep their "s".
    if word.len() > 3
        && word.ends_with('s')
        && !word.ends_with("ss")
        && !word.ends_with("us")
        && !word.ends_with("os")
    {
        return word[..word.len() - 1].to_string();
    }
    word.to_string()
}

fn trigrams(s: &str) -> Vec<[char; 3]> {
    let padded: Vec<char> = format!("  {s} ").chars().collect();
    padded.windows(3).map(|w| [w[0], w[1], w[2]]).collect()
}

/// Dice coefficient over character trigrams, plus a containment bonus so a
/// short name scores high against a longer one that contains it
/// ("quassar" vs "quassar family") — which is exactly the elided-link case.
/// Both inputs must already be normalized.
pub fn similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let (ta, tb) = (trigrams(a), trigrams(b));
    let shared = ta.iter().filter(|t| tb.contains(t)).count();
    let dice = (2.0 * shared as f64) / (ta.len() + tb.len()) as f64;

    // Whole-word containment: every word of the shorter name appears in the
    // longer one. "quassar" ⊂ "quassar family" -> strong signal.
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let long_words: Vec<&str> = long.split_whitespace().collect();
    let contained = short.split_whitespace().all(|w| long_words.contains(&w));

    if contained {
        dice.max(0.75 + 0.25 * dice)
    } else {
        dice
    }
}

/// Find the single best match above `threshold`. Returns `Ambiguous` when more
/// than one candidate clears it — the caller MUST NOT pick a winner.
pub fn best_match<'a>(
    needle: &str,
    haystack: &'a [(String, String)],
    threshold: f64,
) -> MatchOutcome<'a> {
    let n = normalize(needle);
    let mut hits: Vec<(f64, &'a str, &'a str)> = haystack
        .iter()
        .map(|(id, name)| (similarity(&n, &normalize(name)), id.as_str(), name.as_str()))
        .filter(|(score, _, _)| *score >= threshold)
        .collect();

    hits.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    match hits.len() {
        0 => MatchOutcome::None,
        1 => MatchOutcome::Unique {
            id: hits[0].1,
            name: hits[0].2,
            score: hits[0].0,
        },
        _ => MatchOutcome::Ambiguous(
            hits.into_iter()
                .map(|(similarity, id, name)| Candidate {
                    id: id.to_string(),
                    name: name.to_string(),
                    similarity,
                })
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_collapses_the_article_and_plural_variants() {
        // The maintainer's real cases.
        assert_eq!(normalize("The Free League"), normalize("Free League"));
        assert_eq!(normalize("The Quassars"), "quassar");
        assert_eq!(normalize("The Quassar Family"), "quassar family");
        assert_eq!(normalize("Seraphina's Blade"), "seraphina blade");
    }

    #[test]
    fn normalize_does_not_over_collapse() {
        // Indefinite articles are NOT stripped: too many distinct titles start
        // with "A" for that to be safe.
        assert_eq!(normalize("A Cage of Iron"), "a cage of iron");
        // A trailing "s" that is not a plural must survive.
        assert_eq!(normalize("Chaos"), "chaos");
        // "the" only leads; it is not stripped mid-name.
        assert_eq!(normalize("Lord of the Rings"), "lord of the ring");
    }

    #[test]
    fn normalize_is_idempotent() {
        for s in [
            "The Quassars",
            "Chaos",
            "  The   Free  League  ",
            "Ünther's",
        ] {
            assert_eq!(normalize(&normalize(s)), normalize(s), "input: {s}");
        }
    }

    #[test]
    fn similarity_scores_a_partial_name_high_and_a_stranger_low() {
        let quassars = normalize("The Quassars");
        let family = normalize("The Quassar Family");
        assert!(
            similarity(&quassars, &family) > 0.7,
            "partial name must score high"
        );

        // NEGATIVE CASE — these must NOT match. A faction and a tavern.
        // A false merge here is data loss, so this assertion matters more
        // than any positive one.
        let legion = normalize("The Legion");
        let rest = normalize("The Legionnaire's Rest");
        assert!(
            similarity(&legion, &rest) < 0.72,
            "distinct entities must not match"
        );
    }

    #[test]
    fn best_match_refuses_to_guess_when_two_candidates_tie() {
        let haystack = vec![
            ("faction:a".to_string(), "The Quassar Family".to_string()),
            ("faction:b".to_string(), "The Quassar Cartel".to_string()),
        ];
        match best_match("the quassars", &haystack, 0.5) {
            MatchOutcome::Ambiguous(c) => assert_eq!(c.len(), 2),
            other => panic!("ambiguity must never auto-resolve, got {other:?}"),
        }
    }
}
