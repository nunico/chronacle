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
pub fn resolve_exact(link: &str, entities: &[EntityIdentity]) -> Option<String> {
    let lower = link.trim().to_lowercase();

    if let Some(e) = entities.iter().find(|e| e.name.to_lowercase() == lower) {
        return Some(e.id.clone());
    }
    if let Some(e) = entities
        .iter()
        .find(|e| e.aliases.iter().any(|a| a.to_lowercase() == lower))
    {
        return Some(e.id.clone());
    }

    let norm = normalize(link);
    if norm.is_empty() {
        return None;
    }
    entities
        .iter()
        .find(|e| normalize(&e.name) == norm || e.aliases.iter().any(|a| normalize(a) == norm))
        .map(|e| e.id.clone())
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
