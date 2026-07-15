//! YAML frontmatter for vault files.
//!
//! All string scalars are emitted **quoted, unconditionally**. An entity named
//! `Vex: The Unbound` would otherwise emit invalid YAML, and one named
//! `[[Iron Tower]]` would parse as a nested list. `aliases` and `title` are
//! Obsidian-meaningful keys, not private serialisation keys.
//!
//! Every frontmatter field is a single-line scalar by construction —
//! `render()` runs each one through [`chronacle_core::sanitize_scalar`]
//! before quoting. Multi-line content (`summary`, `notes`, `codex_article`)
//! lives in the file body as raw Markdown, never in frontmatter. If a
//! multi-line frontmatter field is ever added, it must use a YAML block
//! scalar (`|`), never a quoted string with escapes.

use chronacle_core::{sanitize_scalar, RulePageRef};
use serde::{Deserialize, Serialize};

/// The closed frontmatter vocabulary. Field order here is emission order.
///
/// Derives `Serialize` for a future `to_value` use case even though
/// `render()` below hand-rolls emission rather than routing through it.
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

/// Wrap a scalar in double quotes, escaping only backslashes and quotes.
///
/// This is sufficient — and provably safe — because every caller runs the
/// value through [`sanitize_scalar`] first, which removes every control
/// character that would otherwise need a YAML escape sequence. The
/// `debug_assert!` below enforces that coupling rather than assuming it: if
/// a caller ever forgets to sanitize first, debug builds fail loudly instead
/// of silently emitting a byte that YAML would fold or drop on the next
/// parse.
fn quote(s: &str) -> String {
    debug_assert!(
        !s.chars().any(char::is_control),
        "quote() received an unsanitized scalar with a control character: {s:?}"
    );
    let mut escaped = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            c => escaped.push(c),
        }
    }
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

    out.push_str(&format!("id: {}\n", quote(&sanitize_scalar(&fm.id))));
    if let Some(name) = &fm.name {
        out.push_str(&format!("name: {}\n", quote(&sanitize_scalar(name))));
    }
    if let Some(title) = &fm.title {
        out.push_str(&format!("title: {}\n", quote(&sanitize_scalar(title))));
    }
    if !fm.aliases.is_empty() {
        let items = fm
            .aliases
            .iter()
            .map(|a| quote(&sanitize_scalar(a)))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("aliases: [{items}]\n"));
    }
    if let Some(kind) = &fm.kind {
        out.push_str(&format!("type: {}\n", quote(&sanitize_scalar(kind))));
    }
    if let Some(campaign) = &fm.campaign {
        out.push_str(&format!(
            "campaign: {}\n",
            quote(&sanitize_scalar(campaign))
        ));
    }
    if let Some(collection) = &fm.collection {
        out.push_str(&format!(
            "collection: {}\n",
            quote(&sanitize_scalar(collection))
        ));
    }
    if let Some(category) = &fm.category {
        out.push_str(&format!(
            "category: {}\n",
            quote(&sanitize_scalar(category))
        ));
    }
    if let Some(session_number) = fm.session_number {
        out.push_str(&format!("session_number: {session_number}\n"));
    }
    if let Some(date_played) = &fm.date_played {
        out.push_str(&format!(
            "date_played: {}\n",
            quote(&sanitize_scalar(date_played))
        ));
    }
    if !fm.page_refs.is_empty() {
        out.push_str("page_refs:\n");
        for page_ref in &fm.page_refs {
            out.push_str(&format!(
                "  - {{ source_name: {}, page_start: {}, page_end: {} }}\n",
                quote(&sanitize_scalar(&page_ref.source_name)),
                page_ref.page_start,
                page_ref.page_end
            ));
        }
    }
    out.push_str(&format!(
        "created_at: {}\n",
        quote(&sanitize_scalar(&fm.created_at))
    ));
    out.push_str(&format!(
        "updated_at: {}\n",
        quote(&sanitize_scalar(&fm.updated_at))
    ));

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

    // Deserialize through a shadow struct with `id: Option<String>` rather
    // than matching on the deserializer's error text: a `yaml_serde` version
    // bump that rewords its "missing field" message would otherwise silently
    // turn `MissingId` into an opaque `Yaml(..)`, changing caller-visible
    // behaviour with no compile-time signal.
    #[derive(Deserialize)]
    struct ShadowFrontmatter {
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        aliases: Vec<String>,
        #[serde(rename = "type", default)]
        kind: Option<String>,
        #[serde(default)]
        campaign: Option<String>,
        #[serde(default)]
        collection: Option<String>,
        #[serde(default)]
        category: Option<String>,
        #[serde(default)]
        session_number: Option<i64>,
        #[serde(default)]
        date_played: Option<String>,
        #[serde(default)]
        page_refs: Vec<RulePageRef>,
        created_at: String,
        updated_at: String,
    }

    let shadow: ShadowFrontmatter =
        yaml_serde::from_str(yaml).map_err(|e| FrontmatterError::Yaml(e.to_string()))?;
    let id = shadow
        .id
        .filter(|id| !id.trim().is_empty())
        .ok_or(FrontmatterError::MissingId)?;

    let fm = Frontmatter {
        id,
        name: shadow.name,
        title: shadow.title,
        aliases: shadow.aliases,
        kind: shadow.kind,
        campaign: shadow.campaign,
        collection: shadow.collection,
        category: shadow.category,
        session_number: shadow.session_number,
        date_played: shadow.date_played,
        page_refs: shadow.page_refs,
        created_at: shadow.created_at,
        updated_at: shadow.updated_at,
    };
    Ok((fm, body.to_owned()))
}

/// Recover the GM's alternate names from a parsed frontmatter `aliases`
/// list: everything except the record's own name (case-insensitive).
///
/// The inverse of `render::frontmatter_aliases`, which put the name into
/// that same list in the first place — solely so Obsidian resolves
/// `[[Name]]`. Without this filter, exporting a record with no GM aliases
/// would parse back as one GM alias equal to its own name, and every
/// inbound apply would fabricate a self-referential alias.
///
/// `name` is `None` for sessions (which have no alternate-name concept);
/// in that case every entry in `aliases` is returned unfiltered, though
/// callers building a `GmParts` for a session should ignore the result —
/// `apply_gm_parts` never writes `aliases` for `session`.
pub fn gm_aliases(name: Option<&str>, aliases: &[String]) -> Vec<String> {
    let name = name.unwrap_or_default();
    aliases
        .iter()
        .filter(|a| !a.eq_ignore_ascii_case(name))
        .cloned()
        .collect()
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
        // Other required fields present, so this isolates the id check
        // itself rather than falling through to a generic Yaml error.
        let file = "---\nname: \"x\"\ncreated_at: \"a\"\nupdated_at: \"b\"\n---\nbody\n";
        assert!(matches!(parse(file), Err(FrontmatterError::MissingId)));
    }

    #[test]
    fn parse_rejects_frontmatter_with_a_blank_id() {
        let file = "---\nid: \"   \"\ncreated_at: \"a\"\nupdated_at: \"b\"\n---\nbody\n";
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

    #[test]
    fn gm_aliases_drops_the_entitys_own_name() {
        let aliases = vec!["Seraphina Aldric".to_string(), "The Archivist".to_string()];
        assert_eq!(
            gm_aliases(Some("Seraphina Aldric"), &aliases),
            vec!["The Archivist".to_string()]
        );
    }

    #[test]
    fn gm_aliases_drops_the_name_case_insensitively() {
        let aliases = vec!["SERAPHINA ALDRIC".to_string(), "The Archivist".to_string()];
        assert_eq!(
            gm_aliases(Some("Seraphina Aldric"), &aliases),
            vec!["The Archivist".to_string()]
        );
    }

    #[test]
    fn gm_aliases_is_empty_when_frontmatter_aliases_contains_only_the_name() {
        let aliases = vec!["Seraphina Aldric".to_string()];
        assert_eq!(
            gm_aliases(Some("Seraphina Aldric"), &aliases),
            Vec::<String>::new()
        );
    }

    #[test]
    fn quote_round_trips_hostile_names_to_their_sanitized_form() {
        // Deliberately changed expectation: hostile inputs no longer survive
        // a round-trip verbatim. Control characters have no semantic value
        // in a single-line field, so render() sanitizes before quoting;
        // the round-trip now lands on the *sanitized* form, not the original.
        let names = [
            "Foo\nBar",
            "Tab\there",
            "Bell\u{7}x",
            "Vex: The Unbound",
            "[[Iron Tower]]",
            "Quote\"inside",
            "Back\\slash",
            "Séraphina 日本語 🗡",
            "\u{7f}del",
        ];
        for name in names {
            let mut fm = entity_fm();
            fm.name = Some(name.to_string());
            let file = format!("{}\nbody\n", render(&fm));
            let (back, _) = parse(&file).unwrap_or_else(|e| panic!("reparse {name:?}: {e}"));
            assert_eq!(
                back.name.as_deref(),
                Some(sanitize_scalar(name).as_str()),
                "round-trip failed for {name:?}"
            );

            // render is a fixed point: render(parse(render(fm))) == render(fm).
            let refile = format!("{}\nbody\n", render(&back));
            assert_eq!(refile, file, "render is not a fixed point for {name:?}");
        }
    }

    #[test]
    fn every_rendered_frontmatter_line_is_single_line() {
        let mut fm = entity_fm();
        fm.name = Some("Hostile\nName\twith\u{7}control".into());
        fm.title = Some("Hostile\nTitle".into());
        fm.aliases = vec!["Hostile\nAlias".into()];
        fm.page_refs = vec![RulePageRef {
            source_name: "Hostile\nSource".into(),
            page_start: 1,
            page_end: 2,
        }];

        let out = render(&fm);
        let yaml = out
            .strip_prefix("---\n")
            .and_then(|rest| rest.strip_suffix("---\n"))
            .expect("well-fenced output");

        for line in yaml.lines() {
            // Every line must either be a top-level `key: value` line or a
            // `  - { ... }` list item — never a bare continuation of a value
            // that spilled onto its own line.
            let is_key_value = line.contains(':') && !line.starts_with(' ');
            let is_list_item = line.starts_with("  - ");
            assert!(
                is_key_value || is_list_item,
                "found a value that spilled onto a continuation line: {line:?}\nfull output:\n{out}"
            );
        }
    }
}
