//! The LLM response schema plus tolerant parsers for the extraction and
//! profile passes, and the kind-string → `EntityKind` mapping.

use serde::Deserialize;

use crate::entity_service::EntityKind;

// ── LLM response schema ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(super) struct LlmResponse {
    #[serde(default)]
    pub(super) entities: Vec<LlmEntity>,
}

#[derive(Debug, Deserialize)]
pub(super) struct LlmEntity {
    pub(super) name: String,
    pub(super) kind: String,
    #[serde(default)]
    pub(super) summary: Option<String>,
    #[serde(default)]
    pub(super) notes: Option<String>,
    #[serde(default)]
    pub(super) relations: Vec<LlmRelation>,
}

#[derive(Debug, Deserialize)]
pub(super) struct LlmRelation {
    pub(super) name: String,
    pub(super) kind: String,
    pub(super) rel_type: String,
    #[serde(default)]
    pub(super) summary: Option<String>,
    #[serde(default)]
    pub(super) notes: Option<String>,
}

/// Summary/notes returned by the profile (enrichment) pass.
#[derive(Debug, Default, Deserialize)]
pub(super) struct ProfileFields {
    #[serde(default)]
    pub(super) summary: Option<String>,
    #[serde(default)]
    pub(super) notes: Option<String>,
}

/// Strip a leading ```json / ``` fence and trailing fence from an LLM response.
fn strip_code_fences(raw: &str) -> &str {
    let trimmed = raw.trim();
    if let Some(s) = trimmed.strip_prefix("```json") {
        s.trim_end_matches("```").trim()
    } else if let Some(s) = trimmed.strip_prefix("```") {
        s.trim_end_matches("```").trim()
    } else {
        trimmed
    }
}

/// Parse a profile-pass response, tolerating markdown fences and malformed JSON.
pub(super) fn parse_profile_response(raw: &str) -> ProfileFields {
    serde_json::from_str(strip_code_fences(raw)).unwrap_or_default()
}

/// Parse the LLM response, tolerating truncated or partially-valid JSON.
pub(super) fn parse_extraction_response(raw: &str) -> LlmResponse {
    serde_json::from_str(strip_code_fences(raw)).unwrap_or_else(|e| {
        eprintln!("extraction: JSON parse failed ({e}), returning empty result");
        LlmResponse { entities: vec![] }
    })
}

/// Convert a kind string from the LLM to an EntityKind, defaulting to Misc.
pub(super) fn parse_kind(kind: &str) -> EntityKind {
    EntityKind::from_table(kind).unwrap_or(EntityKind::Misc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_extraction_response_deserializes_well_formed_json() {
        let json = r#"{
            "entities": [
                {
                    "name": "The Iron Fist",
                    "kind": "faction",
                    "summary": "Militant faction.",
                    "notes": "Key figure: [[Commander Varn]].",
                    "relations": [
                        {
                            "name": "Commander Varn",
                            "kind": "npc",
                            "rel_type": "commands",
                            "summary": "Ruthless leader.",
                            "notes": null
                        }
                    ]
                }
            ]
        }"#;
        let result = parse_extraction_response(json);
        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.entities[0].name, "The Iron Fist");
        assert_eq!(result.entities[0].relations.len(), 1);
        assert_eq!(result.entities[0].relations[0].name, "Commander Varn");
    }

    #[test]
    fn parse_extraction_response_returns_empty_on_malformed_json() {
        let result = parse_extraction_response("not valid json {{{");
        assert!(result.entities.is_empty());
    }

    #[test]
    fn parse_extraction_response_strips_markdown_code_fences() {
        let json = "```json\n{\"entities\":[]}\n```";
        let result = parse_extraction_response(json);
        assert!(result.entities.is_empty());
    }

    #[test]
    fn parse_extraction_response_tolerates_truncated_response() {
        let truncated = r#"{"entities": [{"name": "Foo", "kind": "#;
        let result = parse_extraction_response(truncated);
        assert!(result.entities.is_empty()); // graceful fallback
    }

    #[test]
    fn parse_kind_falls_back_to_misc_for_unknown() {
        assert_eq!(parse_kind("dragon_lord"), EntityKind::Misc);
        assert_eq!(parse_kind("npc"), EntityKind::Npc);
    }

    #[test]
    fn parse_profile_response_extracts_summary_and_notes() {
        let json = r#"{"summary":"A militant faction.","notes":"Led by [[Varn]]."}"#;
        let fields = parse_profile_response(json);
        assert_eq!(fields.summary.as_deref(), Some("A militant faction."));
        assert_eq!(fields.notes.as_deref(), Some("Led by [[Varn]]."));
    }

    #[test]
    fn parse_profile_response_returns_empty_on_malformed_json() {
        let fields = parse_profile_response("not json {{{");
        assert!(fields.summary.is_none());
        assert!(fields.notes.is_none());
    }
}
