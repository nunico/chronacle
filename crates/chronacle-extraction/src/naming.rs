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

/// Tuned against real campaign data (ADR-012). On a real 21-entity campaign,
/// every genuine duplicate was an article/plural variant that normalizes to an
/// *identical* key (caught by stage-1 grouping, independent of this threshold),
/// while the fuzzy 0.85–0.92 band was entirely family/member false positives
/// ("The Quassars" vs "Johar Quassar" = 0.909 — a family and a person in it,
/// which lexical similarity cannot distinguish from a real variant). So the
/// threshold is deliberately HIGH: fuzzy auto-resolve/auto-merge fire only on
/// near-exact matches; the 0.72–0.90 band becomes a reviewable "did you mean?"
/// suggestion (candidates use `DEFAULT_THRESHOLD * 0.8`). Prefer a MISSED match
/// (degrades to a suggestion) over a FALSE one (silently corrupts the graph).
pub const DEFAULT_THRESHOLD: f64 = 0.90;

/// Case-fold, strip a leading "the", drop possessives, singularize a trailing
/// plural, collapse punctuation and whitespace. Never used for storage — only
/// ever as a lookup key. The GM's exact spelling is preserved as name or alias.
pub fn normalize(name: &str) -> String {
    let lowered = name.to_lowercase();

    // Strip a trailing possessive ('s / 's) on each raw (still-punctuated)
    // word BEFORE punctuation is flattened to spaces, so a name that is
    // itself a single character (e.g. "X") is never confused with the "s"
    // left behind by stripping "Seraphina's" -> "Seraphina".
    let depossessed: String = lowered
        .split_whitespace()
        .map(strip_trailing_possessive)
        .collect::<Vec<_>>()
        .join(" ");

    let cleaned: String = depossessed
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

    let result = words
        .iter()
        .map(|w| singularize(w))
        .collect::<Vec<_>>()
        .join(" ");

    if result.is_empty() && !name.is_empty() {
        // Everything was stripped away (e.g. a punctuation-only name like
        // "!!!"). Fall back to the case-folded, whitespace-collapsed
        // original rather than silently collapsing to "" — an empty
        // normalized name would make two unrelated entities match at 1.0.
        return lowered.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    result
}

/// Strip a trailing `'s` or `’s` from a single raw word, e.g.
/// `"seraphina's"` -> `"seraphina"`. Operates before punctuation is
/// flattened so it only ever removes an actual possessive suffix, never a
/// legitimately single-character name.
fn strip_trailing_possessive(word: &str) -> &str {
    word.strip_suffix("'s")
        .or_else(|| word.strip_suffix("\u{2019}s")) // ’s (curly apostrophe)
        .unwrap_or(word)
}

/// Conservative: a small rule set, not a stemmer. Over-eager singularization
/// merges distinct names, which is the expensive direction to be wrong in.
fn singularize(word: &str) -> String {
    // Possessives are stripped earlier, before punctuation flattening (see
    // `strip_trailing_possessive`), so this only ever sees an already-clean
    // word — a lone "s" never reaches here.

    // Words whose singular and plural forms coincide ("series", "species")
    // must never be stemmed — there is no "sery"/"specy" to recover.
    const INVARIANT: [&str; 2] = ["series", "species"];
    if INVARIANT.contains(&word) {
        return word.to_string();
    }

    if word.len() > 3 && word.ends_with("ies") {
        return format!("{}y", &word[..word.len() - 3]);
    }
    if word.len() > 3 && word.ends_with("es") && !word.ends_with("ses") {
        return word[..word.len() - 2].to_string();
    }
    // "chaos", "ss"/"us"/"os" endings, and a trailing "s" preceded by "a" or
    // "i" ("Atlas", "Silas", "Iris") are real names, not plurals — keep the
    // "s". Under-stemming only misses a match (a suggestion the GM can
    // accept); over-stemming silently fuses two distinct names.
    if word.len() > 3
        && word.ends_with('s')
        && !word.ends_with("ss")
        && !word.ends_with("us")
        && !word.ends_with("os")
        && !word.ends_with("as")
        && !word.ends_with("is")
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
    // The empty check MUST precede the equality check: two inputs that both
    // normalize to "" would otherwise short-circuit to a "perfect" 1.0 match
    // between two unrelated entities.
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }

    let (ta, tb) = (trigrams(a), trigrams(b));
    // Multiset intersection (min of per-trigram counts on each side), NOT a
    // membership test against the other's Vec: a plain `tb.contains(t)`
    // check double-counts repeated trigrams asymmetrically (e.g. "iron
    // iron" vs "iron fist") and silently breaks similarity(a, b) ==
    // similarity(b, a) — which this module exists to guarantee.
    let mut counts_a: std::collections::HashMap<[char; 3], usize> =
        std::collections::HashMap::new();
    for t in &ta {
        *counts_a.entry(*t).or_insert(0) += 1;
    }
    let mut counts_b: std::collections::HashMap<[char; 3], usize> =
        std::collections::HashMap::new();
    for t in &tb {
        *counts_b.entry(*t).or_insert(0) += 1;
    }
    let shared: usize = counts_a
        .iter()
        .map(|(t, &n)| n.min(*counts_b.get(t).unwrap_or(&0)))
        .sum();
    let dice = (2.0 * shared as f64) / (ta.len() + tb.len()) as f64;

    // Whole-word containment: every word of one name appears in the other.
    // "quassar" ⊂ "quassar family" -> strong signal. Checked in BOTH
    // directions and the max taken, so the result never depends on which
    // argument came first — resolution and duplicate detection must never
    // disagree about whether two names are "the same".
    let a_words: Vec<&str> = a.split_whitespace().collect();
    let b_words: Vec<&str> = b.split_whitespace().collect();
    let a_in_b = a_words.iter().all(|w| b_words.contains(w));
    let b_in_a = b_words.iter().all(|w| a_words.contains(w));

    if a_in_b || b_in_a {
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
            similarity(&legion, &rest) < DEFAULT_THRESHOLD,
            "distinct entities must not match"
        );
    }

    #[test]
    fn normalize_never_empties_a_non_empty_input() {
        assert_eq!(normalize("X"), "x");
        assert_eq!(normalize("K"), "k");
        assert_ne!(normalize("!!!"), "");
    }

    #[test]
    fn similarity_of_two_distinct_single_char_names_is_not_perfect() {
        // Both normalize to non-empty single characters that differ; must
        // NOT hit the a == b short circuit via a shared "" collapse.
        assert!(similarity(&normalize("X"), &normalize("K")) < 1.0);
        assert!(similarity(&normalize("!!!"), &normalize("???")) < 1.0);
    }

    #[test]
    fn similarity_of_empty_inputs_is_zero_not_one() {
        assert_eq!(similarity("", ""), 0.0);
    }

    #[test]
    fn similarity_is_symmetric() {
        let pairs = [
            ("quassar", "quassar family"),
            ("legion", "legionnaire rest"),
            ("iron iron", "iron fist"),
            ("a", "ab"),
            ("chaos", "chaotic"),
        ];
        for (a, b) in pairs {
            assert_eq!(
                similarity(a, b),
                similarity(b, a),
                "similarity must be symmetric for ({a:?}, {b:?})"
            );
        }
    }

    #[test]
    fn singularize_does_not_mangle_as_is_names() {
        assert_eq!(normalize("Atlas"), "atlas");
        assert_eq!(normalize("Silas"), "silas");
        assert_eq!(normalize("Iris"), "iris");
        assert_eq!(normalize("Series"), "series");
        assert_eq!(normalize("Species"), "species");
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
