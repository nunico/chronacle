//! Prompt construction for entity extraction and the second-pass profile
//! enrichment, plus the shared kind / relationship-type vocabulary blocks.

/// Per-kind definitions shared by the classifying prompts. Keeps the two prompts
/// in sync (DRY); encodes the invariant that organized groups and families are
/// factions, not npcs.
const ENTITY_KIND_DEFS: &str = "Entity kinds (choose the single best fit):
- npc: a single named individual (person, named monster, deity).
- location: a place — region, settlement, building, room, or plane.
- faction: any organized group of people — guild, cult, government, military order, crime ring, noble house, family, or clan.
- creature: a kind or species of being, not a named individual (e.g. \"goblin\", \"owlbear\").
- item: an object, artifact, weapon, or substance.
- event: something that happens at a point or span in time.
- player_character: a player character (PC) controlled by a player.
- misc: anything that fits none of the above.";

/// Canonical relationship vocabulary shared by the classifying prompts. Mirrors
/// the `RelType` variants so the LLM emits values that normalize cleanly.
const REL_TYPE_VOCAB: &str = "Relationship types. Emit EXACTLY ONE snake_case value from the list below (e.g. `leads` OR `led_by` — never both, never the slash, never invent a new one unless none fits). Pick the side whose direction matches the sentence. Only if nothing fits, use a short snake_case verb:
- leads / led_by: X leads or commands group Y (led_by is the inverse).
- member_of / has_member: X belongs to group or family Y.
- located_in / contains: X is situated within place Y.
- owns / owned_by: X owns or possesses Y.
- serves / served_by: X serves or is loyal to Y.
- created / created_by: X created or founded Y.
- parent_of / child_of: X is a parent or ancestor of Y.
- allied_with: X and Y are allied (no direction).
- enemy_of: X and Y are enemies (no direction).
- related_to: X and Y are kin or otherwise associated (no direction).
- knows: X and Y are acquainted (no direction).";

/// Build the system prompt that instructs the LLM to extract entities as JSON.
pub(super) fn build_extraction_prompt(chunk_text: &str) -> String {
    format!(
        r#"You are an expert at extracting structured game entities from TTRPG source material.

Extract all named entities from the following text. For each entity:
- Identify its kind using these definitions:
{ENTITY_KIND_DEFS}
- For entities directly related to a level-0 entity, include them in that entity's "relations" array
- For entities mentioned only in passing (level 2+), write their names as [[wikilinks]] inside the notes field — do NOT extract them as separate entities

Field rules (apply to BOTH top-level entities and entities in "relations"):
- "summary": a short, concise description of the entity ITSELF — who or what it is — in 1 sentence. Do NOT describe how it relates to any other entity here. The ONLY exception is when the entity is inherently about a relationship (e.g. an association, alliance, or pact between parties); then the relationship is its identity and belongs in the summary.
- "notes": a more thorough description, including how this entity relates to others (its role, ties, and the connection to the entity it was extracted alongside). May contain [[wikilinks]]. Leave empty if there is nothing beyond the summary.

{REL_TYPE_VOCAB}

Return ONLY valid JSON matching this exact schema (no markdown, no explanation):

{{
  "entities": [
    {{
      "name": "string",
      "kind": "npc|location|faction|creature|item|event|player_character|misc",
      "summary": "short, concise description of the entity itself",
      "notes": "optional longer description incl. relationships, may contain [[wikilinks]]",
      "relations": [
        {{
          "name": "string",
          "kind": "string",
          "rel_type": "exactly one snake_case type from the list below, e.g. leads",
          "summary": "short, concise description of this entity itself — NOT its relation to the parent",
          "notes": "optional longer description incl. how it relates to the parent entity"
        }}
      ]
    }}
  ]
}}

Source text:
{chunk_text}"#
    )
}

/// Build a seed-anchored extraction prompt: focus on `name` and the entities
/// directly related to it, rather than extracting everything in the text.
pub(super) fn build_seed_prompt(name: &str, chunk_text: &str) -> String {
    format!(
        r#"You are an expert at extracting structured game entities from TTRPG source material.

Build a complete profile of the entity named "{name}" using ONLY the source text below.
- Output "{name}" as a single level-0 entity with its kind, summary, and notes.
- Classify every entity's kind using these definitions:
{ENTITY_KIND_DEFS}
- Include entities DIRECTLY related to "{name}" in its "relations" array (allies, members, locations, leaders, etc.).
- For entities mentioned only in passing, write their names as [[wikilinks]] inside notes — do NOT extract them separately.
- If "{name}" is not described in the text, return an empty "entities" array.

Field rules (apply to BOTH "{name}" and entities in "relations"):
- "summary": a short, concise description of the entity ITSELF — who or what it is — in 1 sentence. Do NOT describe how a related entity connects to "{name}" here. The ONLY exception is when the entity is inherently about a relationship (e.g. an association, alliance, or pact between parties); then the relationship is its identity and belongs in the summary.
- "notes": a more thorough description, including how the entity relates to "{name}" (its role, ties, and connection). May contain [[wikilinks]]. Leave empty if there is nothing beyond the summary.

{REL_TYPE_VOCAB}

Return ONLY valid JSON matching this exact schema (no markdown, no explanation):

{{
  "entities": [
    {{
      "name": "string",
      "kind": "npc|location|faction|creature|item|event|player_character|misc",
      "summary": "short, concise description of the entity itself",
      "notes": "optional longer description incl. relationships, may contain [[wikilinks]]",
      "relations": [
        {{ "name": "string", "kind": "string", "rel_type": "exactly one snake_case type from the list below, e.g. leads", "summary": "short, concise description of this entity itself — NOT its relation to {name}", "notes": "optional longer description incl. how it relates to {name}" }}
      ]
    }}
  ]
}}

Source text:
{chunk_text}"#
    )
}

/// Build a profile prompt for the second enrichment pass: describe ONE entity
/// from its own passages, with no relations (depth-1, description only).
pub(super) fn build_profile_prompt(name: &str, chunk_text: &str) -> String {
    format!(
        r#"You are an expert at describing game entities from TTRPG source material.

Describe ONLY the entity named "{name}" using the source text below.
- "summary": a short, concise description of "{name}" ITSELF — who or what it is — in 1 sentence. Do NOT describe how it relates to other entities, UNLESS "{name}" is inherently about a relationship (e.g. an association, alliance, or pact between parties), in which case that relationship is its identity and belongs here.
- "notes": a more thorough description, including how "{name}" relates to others. May contain [[wikilinks]]. Use an empty string if there is nothing beyond the summary.
- If "{name}" is not described in the text, return empty strings.

Do NOT extract any other entities or relations.

Return ONLY valid JSON matching this exact schema (no markdown, no explanation):

{{ "summary": "string", "notes": "string" }}

Source text:
{chunk_text}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_extraction_prompt_contains_chunk_text() {
        let prompt = build_extraction_prompt("The Iron Fist faction rules the docks.");
        assert!(prompt.contains("The Iron Fist faction rules the docks."));
        assert!(prompt.contains("entities"));
        assert!(prompt.contains("JSON"));
    }

    #[test]
    fn build_seed_prompt_anchors_on_entity_name() {
        let prompt = build_seed_prompt("Commander Varn", "Varn leads the Iron Fist.");
        assert!(prompt.contains("Commander Varn"));
        assert!(prompt.contains("Varn leads the Iron Fist."));
        assert!(prompt.contains("entities"));
        assert!(prompt.contains("JSON"));
    }

    #[test]
    fn extraction_prompt_defines_kinds_and_rel_vocab() {
        let prompt = build_extraction_prompt("any text");
        assert!(prompt.contains("faction:"));
        assert!(prompt.contains("noble house, family, or clan"));
        assert!(prompt.contains("creature:"));
        assert!(prompt.contains("player_character:"));
        assert!(prompt.contains("leads / led_by"));
        assert!(prompt.contains("parent_of / child_of"));
        assert!(prompt.contains("allied_with"));
        // The LLM must be told to emit a single snake_case token, not the slash pair.
        assert!(prompt.contains("EXACTLY ONE snake_case value"));
    }

    #[test]
    fn seed_prompt_defines_kinds_and_rel_vocab() {
        let prompt = build_seed_prompt("Varn", "Varn leads the Iron Fist.");
        assert!(prompt.contains("noble house, family, or clan"));
        assert!(prompt.contains("leads / led_by"));
        assert!(prompt.contains("parent_of / child_of"));
        assert!(prompt.contains("EXACTLY ONE snake_case value"));
    }

    #[test]
    fn profile_prompt_omits_kind_and_rel_vocab() {
        // The enrichment/profile pass describes one entity — it classifies no
        // kinds and extracts no relations, so it must NOT carry either block.
        let prompt = build_profile_prompt("Varn", "Varn leads the Iron Fist.");
        assert!(!prompt.contains("Entity kinds"));
        assert!(!prompt.contains("Relationship types"));
    }

    #[test]
    fn build_profile_prompt_anchors_on_name_and_omits_relations() {
        let prompt = build_profile_prompt("The Iron Fist", "The Iron Fist rules the docks.");
        assert!(prompt.contains("The Iron Fist"));
        assert!(prompt.contains("The Iron Fist rules the docks."));
        assert!(prompt.contains("summary"));
        assert!(prompt.contains("notes"));
        // The profile pass must NOT ask for relations (depth-1, description only).
        assert!(!prompt.contains("\"relations\""));
    }
}
