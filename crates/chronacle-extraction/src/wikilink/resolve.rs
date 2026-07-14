//! Tiered wikilink resolution. Tiers 1-3 are deterministic (this file); tier 4
//! (fuzzy) lives in `mod.rs` because it has side effects — it writes an alias.

use crate::naming::normalize;

/// An entity as the resolver sees it: identity plus every name it answers to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityIdentity {
    /// Full record id, e.g. `"npc:abc123"`.
    pub id: String,
    /// The entity's canonical, GM-authored name.
    pub name: String,
    /// Confirmed alternate names — matched exactly, same as `name`.
    pub aliases: Vec<String>,
}

/// Tiers 1-3, in order, first hit wins:
///   1. exact name (case-insensitive)
///   2. exact alias (case-insensitive)   — a confirmed variant, forever
///   3. normalized name or alias         — "Free League" == "The Free League"
///
/// Returns the full record id. A tier-3 match is still EXACT — on a normalized
/// key — so there is no threshold and no ambiguity to adjudicate here.
///
/// Within a single tier, more than one entity can legitimately match (e.g.
/// two factions whose names both normalize to "free league"). That is
/// genuinely ambiguous GM data, not something this function can resolve
/// correctly — so it does not try. Instead it breaks the tie by picking the
/// entity with the lexicographically smallest full record id, purely so the
/// same link always resolves to the same entity across runs and machines.
/// Such collisions are meant to be caught separately and surfaced to the GM
/// as an `alias_collision` lint finding (a later task), not silently guessed
/// at here.
pub fn resolve_exact(link: &str, entities: &[EntityIdentity]) -> Option<String> {
    let lower = link.trim().to_lowercase();

    if let Some(id) = smallest_id(entities.iter().filter(|e| e.name.to_lowercase() == lower)) {
        return Some(id);
    }
    if let Some(id) = smallest_id(
        entities
            .iter()
            .filter(|e| e.aliases.iter().any(|a| a.to_lowercase() == lower)),
    ) {
        return Some(id);
    }

    let norm = normalize(link);
    if norm.is_empty() {
        return None;
    }
    smallest_id(
        entities.iter().filter(|e| {
            normalize(&e.name) == norm || e.aliases.iter().any(|a| normalize(a) == norm)
        }),
    )
}

/// Deterministic within-tier tie-break: the lexicographically smallest full
/// record id among the candidates, if any.
fn smallest_id<'a>(candidates: impl Iterator<Item = &'a EntityIdentity>) -> Option<String> {
    candidates.map(|e| e.id.clone()).min()
}

#[cfg(test)]
mod tests {
    use super::resolve_exact;
    use super::EntityIdentity;

    fn fixture() -> Vec<EntityIdentity> {
        vec![
            EntityIdentity {
                id: "faction:fl".into(),
                name: "The Free League".into(),
                aliases: vec![],
            },
            EntityIdentity {
                id: "npc:s".into(),
                name: "Seraphina Aldric".into(),
                aliases: vec!["Sera".into()],
            },
        ]
    }

    #[test]
    fn tier_1_exact_name_still_wins() {
        assert_eq!(
            resolve_exact("the free league", &fixture()).as_deref(),
            Some("faction:fl")
        );
    }

    #[test]
    fn tier_2_matches_a_confirmed_alias() {
        assert_eq!(resolve_exact("Sera", &fixture()).as_deref(), Some("npc:s"));
    }

    /// Adversarial: an entity literally NAMED "Sera" must beat a different
    /// entity that merely has "Sera" as an alias. Unlike
    /// `tier_1_exact_name_still_wins`, tier 1 and tier 3 disagree here — this
    /// can only pass if tier 1 actually runs, and runs first.
    #[test]
    fn tier_1_beats_tier_2_when_they_disagree() {
        let entities = vec![
            EntityIdentity {
                id: "npc:a".into(),
                name: "Sera".into(),
                aliases: vec![],
            },
            EntityIdentity {
                id: "npc:b".into(),
                name: "Seraphina".into(),
                aliases: vec!["Sera".into()],
            },
        ];
        assert_eq!(resolve_exact("Sera", &entities).as_deref(), Some("npc:a"));
    }

    /// Adversarial: an exact confirmed alias must beat a different entity
    /// whose NAME merely normalizes to the same key. Tier 2 and tier 3
    /// disagree here — this can only pass if tier 2 actually runs before
    /// tier 3.
    #[test]
    fn tier_2_beats_tier_3_when_they_disagree() {
        let entities = vec![
            EntityIdentity {
                id: "faction:c".into(),
                name: "The Ash Court".into(),
                aliases: vec!["Emberguard".into()],
            },
            EntityIdentity {
                id: "faction:d".into(),
                name: "Emberguards".into(),
                aliases: vec![],
            },
        ];
        assert_eq!(
            resolve_exact("Emberguard", &entities).as_deref(),
            Some("faction:c")
        );
    }

    /// Within a single tier, two entities can genuinely collide (both names
    /// are equal case-insensitively — a data-entry mistake, but a real one).
    /// The tie-break must be deterministic regardless of input order — pick
    /// the lexicographically smallest id.
    #[test]
    fn within_tier_collision_breaks_ties_by_smallest_id() {
        let forward = vec![
            EntityIdentity {
                id: "faction:z".into(),
                name: "The Free League".into(),
                aliases: vec![],
            },
            EntityIdentity {
                id: "faction:a".into(),
                name: "THE FREE LEAGUE".into(),
                aliases: vec![],
            },
        ];
        let mut reversed = forward.clone();
        reversed.reverse();

        assert_eq!(
            resolve_exact("The Free League", &forward).as_deref(),
            Some("faction:a")
        );
        assert_eq!(
            resolve_exact("The Free League", &reversed).as_deref(),
            Some("faction:a")
        );
    }

    #[test]
    fn tier_3_matches_across_the_leading_article() {
        // The maintainer's case: the notes say "Free League", the entity is
        // "The Free League". Both normalize to "free league" -> EXACT match on
        // a normalized key. No threshold involved.
        assert_eq!(
            resolve_exact("Free League", &fixture()).as_deref(),
            Some("faction:fl")
        );
    }

    #[test]
    fn an_unrelated_name_does_not_resolve() {
        assert_eq!(resolve_exact("Iron Host", &fixture()), None);
    }
}
