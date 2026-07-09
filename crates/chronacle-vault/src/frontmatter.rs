//! YAML frontmatter for vault files.
//!
//! All string scalars are emitted **quoted, unconditionally**. An entity named
//! `Vex: The Unbound` would otherwise emit invalid YAML, and one named
//! `[[Iron Tower]]` would parse as a nested list. `aliases` and `title` are
//! Obsidian-meaningful keys, not private serialisation keys.

use chronacle_core::RulePageRef;
use serde::{Deserialize, Serialize};

/// The closed frontmatter vocabulary. Field order here is emission order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frontmatter {
    /// Stable record identity, e.g. `"npc:abc123"`. Never derived from the path.
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Obsidian display name. Set for every record; the filename is a slug.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Makes `[[Name]]` resolve to a slug-named file. Without this every
    /// compiled wikilink renders broken in Obsidian.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campaign: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_number: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_played: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub page_refs: Vec<RulePageRef>,
    pub created_at: String,
    pub updated_at: String,
}

/// Errors from parsing a vault file's frontmatter.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FrontmatterError {
    /// The file does not open with a `---` fence.
    #[error("file has no frontmatter")]
    Missing,
    /// Frontmatter present but carries no `id` — cannot be identified.
    #[error("frontmatter has no id")]
    MissingId,
    /// The YAML between the fences did not parse.
    #[error("invalid YAML: {0}")]
    Yaml(String),
}

/// Wrap a scalar in double quotes, escaping embedded backslashes and quotes.
///
/// Escaping order matters: backslashes must be escaped before quotes, or a
/// `\"` produced by the quote-escape step would itself get re-escaped.
fn quote(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Render frontmatter, including the opening and closing `---` fences.
///
/// Trailing newline is included, so a caller appends the body directly.
///
/// `Frontmatter` is a closed struct with a fixed field set and emission
/// order, so this renders each present field by hand rather than routing
/// through `yaml_serde`'s writer: that path has no public API to force
/// quoted scalar emission, and post-processing its output is fragile
/// against colons and multi-line values. `yaml_serde::from_str` is used for
/// the parse direction below, which is where a real YAML parser is needed.
pub fn render(fm: &Frontmatter) -> String {
    let mut out = String::from("---\n");

    out.push_str(&format!("id: {}\n", quote(&fm.id)));
    if let Some(name) = &fm.name {
        out.push_str(&format!("name: {}\n", quote(name)));
    }
    if let Some(title) = &fm.title {
        out.push_str(&format!("title: {}\n", quote(title)));
    }
    if !fm.aliases.is_empty() {
        let items = fm
            .aliases
            .iter()
            .map(|a| quote(a))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("aliases: [{items}]\n"));
    }
    if let Some(kind) = &fm.kind {
        out.push_str(&format!("type: {}\n", quote(kind)));
    }
    if let Some(campaign) = &fm.campaign {
        out.push_str(&format!("campaign: {}\n", quote(campaign)));
    }
    if let Some(collection) = &fm.collection {
        out.push_str(&format!("collection: {}\n", quote(collection)));
    }
    if let Some(category) = &fm.category {
        out.push_str(&format!("category: {}\n", quote(category)));
    }
    if let Some(session_number) = fm.session_number {
        out.push_str(&format!("session_number: {session_number}\n"));
    }
    if let Some(date_played) = &fm.date_played {
        out.push_str(&format!("date_played: {}\n", quote(date_played)));
    }
    if !fm.page_refs.is_empty() {
        out.push_str("page_refs:\n");
        for page_ref in &fm.page_refs {
            out.push_str(&format!(
                "  - {{ source_name: {}, page_start: {}, page_end: {} }}\n",
                quote(&page_ref.source_name),
                page_ref.page_start,
                page_ref.page_end
            ));
        }
    }
    out.push_str(&format!("created_at: {}\n", quote(&fm.created_at)));
    out.push_str(&format!("updated_at: {}\n", quote(&fm.updated_at)));

    out.push_str("---\n");
    out
}

/// Split a vault file into its frontmatter and its body.
///
/// The body is returned verbatim, including its leading newline — the caller
/// (`markdown::split_body`) owns all further structure.
pub fn parse(file: &str) -> Result<(Frontmatter, String), FrontmatterError> {
    let rest = file
        .strip_prefix("---\n")
        .ok_or(FrontmatterError::Missing)?;
    let end = rest.find("\n---\n").ok_or(FrontmatterError::Missing)?;
    let (yaml, body) = rest.split_at(end);
    let body = &body["\n---\n".len()..];

    let fm: Frontmatter = yaml_serde::from_str(yaml).map_err(|e| {
        let msg = e.to_string();
        if msg.contains("missing field `id`") {
            FrontmatterError::MissingId
        } else {
            FrontmatterError::Yaml(msg)
        }
    })?;
    if fm.id.trim().is_empty() {
        return Err(FrontmatterError::MissingId);
    }
    Ok((fm, body.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn entity_fm() -> Frontmatter {
        Frontmatter {
            id: "npc:abc123".into(),
            name: Some("Seraphina Aldric".into()),
            title: Some("Seraphina Aldric".into()),
            aliases: vec!["Seraphina Aldric".into()],
            kind: Some("npc".into()),
            campaign: Some("Shadows of Valdris".into()),
            collection: None,
            category: None,
            session_number: None,
            date_played: None,
            page_refs: vec![],
            created_at: "2026-05-28T14:00:00Z".into(),
            updated_at: "2026-07-09T18:32:00Z".into(),
        }
    }

    #[test]
    fn render_emits_fenced_yaml_with_id_first() {
        let out = render(&entity_fm());
        assert!(out.starts_with("---\n"), "must open with a YAML fence");
        assert!(out.ends_with("---\n"), "must close with a YAML fence");
        let first_key = out.lines().nth(1).unwrap();
        assert!(
            first_key.starts_with("id:"),
            "id must be first, got {first_key:?}"
        );
    }

    #[test]
    fn render_quotes_every_string_scalar() {
        let out = render(&entity_fm());
        assert!(out.contains(r#"id: "npc:abc123""#));
        assert!(out.contains(r#"name: "Seraphina Aldric""#));
        assert!(out.contains(r#"type: "npc""#));
    }

    #[test]
    fn render_quotes_a_name_containing_a_colon() {
        let mut fm = entity_fm();
        fm.name = Some("Vex: The Unbound".into());
        let out = render(&fm);
        assert!(out.contains(r#"name: "Vex: The Unbound""#), "got:\n{out}");
        // and it must survive a round-trip
        let (back, _) = parse(&format!("{out}\nbody")).expect("reparse");
        assert_eq!(back.name.as_deref(), Some("Vex: The Unbound"));
    }

    #[test]
    fn render_quotes_a_name_that_looks_like_a_wikilink() {
        let mut fm = entity_fm();
        fm.name = Some("[[Iron Tower]]".into());
        let out = render(&fm);
        let (back, _) = parse(&format!("{out}\nbody")).expect("reparse");
        assert_eq!(back.name.as_deref(), Some("[[Iron Tower]]"));
    }

    #[test]
    fn parse_splits_frontmatter_from_body() {
        let file =
            "---\nid: \"npc:a\"\ncreated_at: \"x\"\nupdated_at: \"y\"\n---\n\n## Notes\n\nhi\n";
        let (fm, body) = parse(file).expect("parse");
        assert_eq!(fm.id, "npc:a");
        assert_eq!(body, "\n## Notes\n\nhi\n");
    }

    #[test]
    fn parse_rejects_a_file_with_no_frontmatter() {
        assert!(matches!(
            parse("## Notes\n"),
            Err(FrontmatterError::Missing)
        ));
    }

    #[test]
    fn parse_rejects_frontmatter_with_no_id() {
        let file = "---\nname: \"x\"\n---\nbody\n";
        assert!(matches!(parse(file), Err(FrontmatterError::MissingId)));
    }

    #[test]
    fn render_then_parse_round_trips() {
        let fm = entity_fm();
        let file = format!("{}\nbody\n", render(&fm));
        let (back, body) = parse(&file).expect("parse");
        assert_eq!(back, fm);
        assert_eq!(body, "\nbody\n");
    }

    #[test]
    fn aliases_survive_a_round_trip_as_a_list() {
        let mut fm = entity_fm();
        fm.aliases = vec!["A".into(), "B".into()];
        let file = format!("{}\nbody\n", render(&fm));
        let (back, _) = parse(&file).expect("parse");
        assert_eq!(back.aliases, vec!["A".to_string(), "B".to_string()]);
    }
}
