//! Render a record to its full vault file, and hash file content.

use std::hash::{Hash, Hasher};

use chronacle_core::{VaultRecord, VaultScope};

use crate::frontmatter::Frontmatter;
use crate::markdown::{self, BodyParts};

/// Hash normalized content. A merge/loop guard, **not** a security primitive —
/// which is why this is `std`'s `DefaultHasher` and not a new crate dependency.
pub fn content_hash(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    markdown::normalize(s).hash(&mut h);
    h.finish()
}

/// Merge a record's own display name with its GM-authored alternate names
/// into the single frontmatter `aliases` list: name first, then `aliases`,
/// deduplicated case-insensitively.
///
/// The frontmatter `aliases` key is Obsidian-meaningful (`[[Name]]` link
/// resolution requires the name itself to be present) as well as the seam
/// the GM's alternate names round-trip through, so both must live in the
/// same list, with the canonical name always present and always first. The
/// inverse — recovering just the GM's alternate names from a parsed file —
/// is `frontmatter::gm_aliases`.
fn frontmatter_aliases(name: &str, aliases: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(aliases.len() + 1);
    for candidate in std::iter::once(name).chain(aliases.iter().map(String::as_str)) {
        if seen.insert(candidate.to_ascii_lowercase()) {
            out.push(candidate.to_owned());
        }
    }
    out
}

/// Render a record to its complete `.md` file: frontmatter plus body.
pub fn render_record(record: &VaultRecord) -> String {
    let (fm, parts) = match record {
        VaultRecord::Entity(e) => (
            Frontmatter {
                id: e.vref.to_thing(),
                name: Some(e.name.clone()),
                title: Some(e.name.clone()),
                // Without the name in this list, `[[Seraphina Aldric]]` in a
                // compiled article renders broken: wikilinks resolve against
                // `name`, files are slug-named. GM alternate names ride
                // along in the same list — see `frontmatter_aliases`.
                aliases: frontmatter_aliases(&e.name, &e.aliases),
                kind: Some(e.vref.table.clone()),
                campaign: scope_campaign_name(&e.scope),
                collection: scope_collection_name(&e.scope),
                category: None,
                session_number: None,
                date_played: None,
                page_refs: vec![],
                created_at: e.created_at.clone(),
                updated_at: e.updated_at.clone(),
            },
            BodyParts {
                summary: e.summary.clone(),
                fenced: e.codex_article.clone(),
                notes: e.notes.clone(),
            },
        ),
        // Sessions carry no compiled body, so there is no fence: the whole
        // body is GM-owned `notes`. `title` is both our field and Obsidian's.
        VaultRecord::Session(s) => (
            Frontmatter {
                id: s.vref.to_thing(),
                name: None,
                title: Some(s.title.clone()),
                aliases: vec![s.title.clone()],
                kind: None,
                campaign: scope_campaign_name(&s.campaign),
                collection: None,
                category: None,
                session_number: Some(s.session_number),
                date_played: Some(s.date_played.clone()),
                page_refs: vec![],
                created_at: s.created_at.clone(),
                updated_at: s.updated_at.clone(),
            },
            BodyParts {
                summary: None,
                fenced: None,
                notes: Some(s.notes.clone()),
            },
        ),
        // Rule entries mirror the entity split: `body` is compiler-owned and
        // fenced; `notes` is GM-owned. `page_refs` is read-only provenance.
        VaultRecord::RuleEntry(r) => (
            Frontmatter {
                id: r.vref.to_thing(),
                name: Some(r.name.clone()),
                title: Some(r.name.clone()),
                aliases: frontmatter_aliases(&r.name, &r.aliases),
                kind: None,
                campaign: None,
                collection: scope_collection_name(&r.collection),
                category: Some(r.category.clone()),
                session_number: None,
                date_played: None,
                page_refs: r.page_refs.clone(),
                created_at: r.created_at.clone(),
                updated_at: r.updated_at.clone(),
            },
            BodyParts {
                summary: None,
                fenced: Some(r.body.clone()),
                notes: r.notes.clone(),
            },
        ),
    };
    format!(
        "{}{}",
        crate::frontmatter::render(&fm),
        markdown::render_body(&parts)
    )
}

/// The campaign name, when this scope is a campaign.
fn scope_campaign_name(scope: &VaultScope) -> Option<String> {
    match scope {
        VaultScope::Campaign { name, .. } => Some(name.clone()),
        VaultScope::Collection { .. } => None,
    }
}

/// The collection name, when this scope is a collection.
fn scope_collection_name(scope: &VaultScope) -> Option<String> {
    match scope {
        VaultScope::Collection { name, .. } => Some(name.clone()),
        VaultScope::Campaign { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronacle_core::{EntityRecord, SessionRecord, VaultRecord, VaultRef, VaultScope};

    fn npc() -> VaultRecord {
        VaultRecord::Entity(EntityRecord {
            vref: VaultRef {
                table: "npc".into(),
                id: "abc123".into(),
            },
            name: "Seraphina Aldric".into(),
            summary: Some("Archivist of the Iron Tower.".into()),
            notes: Some("GM notes.".into()),
            codex_article: Some("Seraphina is the archivist of [[The Iron Tower]].".into()),
            aliases: vec![],
            scope: VaultScope::Campaign {
                id: "campaign:c1".into(),
                name: "Shadows of Valdris".into(),
            },
            created_at: "2026-05-28T14:00:00Z".into(),
            updated_at: "2026-07-09T18:32:00Z".into(),
        })
    }

    #[test]
    fn render_record_emits_aliases_so_obsidian_wikilinks_resolve() {
        let out = render_record(&npc());
        assert!(
            out.contains(r#"aliases: ["Seraphina Aldric"]"#),
            "got:\n{out}"
        );
        assert!(out.contains(r#"title: "Seraphina Aldric""#));
    }

    #[test]
    fn render_record_puts_gm_alternate_names_in_frontmatter_aliases_name_first() {
        let VaultRecord::Entity(mut e) = npc() else {
            unreachable!()
        };
        e.aliases = vec!["The Archivist".into(), "Seraphina".into()];
        let out = render_record(&VaultRecord::Entity(e));
        assert!(
            out.contains(r#"aliases: ["Seraphina Aldric", "The Archivist", "Seraphina"]"#),
            "got:\n{out}"
        );
    }

    #[test]
    fn render_record_dedups_aliases_case_insensitively_keeping_the_name_first() {
        let VaultRecord::Entity(mut e) = npc() else {
            unreachable!()
        };
        // A GM alternate name that only differs in case from the entity's
        // own name must not produce a duplicate frontmatter entry.
        e.aliases = vec!["seraphina aldric".into(), "The Archivist".into()];
        let out = render_record(&VaultRecord::Entity(e));
        assert!(
            out.contains(r#"aliases: ["Seraphina Aldric", "The Archivist"]"#),
            "got:\n{out}"
        );
    }

    /// The full round trip the seam exists for: export a record carrying GM
    /// alternate names, parse the rendered file back, recover the GM's
    /// aliases with `frontmatter::gm_aliases` (name excluded), and confirm
    /// they equal the original DB aliases. Then re-render from a record
    /// updated with those recovered aliases and confirm the output is
    /// byte-identical to the first export — the seam is a fixed point, not
    /// just a one-way transform.
    #[test]
    fn aliases_round_trip_idempotently_through_export_and_parse() {
        let VaultRecord::Entity(mut e) = npc() else {
            unreachable!()
        };
        e.aliases = vec!["The Archivist".into(), "Seraphina".into()];
        let record = VaultRecord::Entity(e.clone());

        let exported = render_record(&record);
        let (fm, _body) = crate::frontmatter::parse(&exported).expect("parse rendered file");
        let recovered = crate::frontmatter::gm_aliases(fm.name.as_deref(), &fm.aliases);
        assert_eq!(
            recovered, e.aliases,
            "recovered GM aliases must equal the original DB aliases, name excluded"
        );

        let mut re_entity = e.clone();
        re_entity.aliases = recovered;
        let re_exported = render_record(&VaultRecord::Entity(re_entity));
        assert_eq!(
            re_exported, exported,
            "re-export from the recovered aliases must be byte-identical"
        );
    }

    /// A file whose frontmatter `aliases` contains ONLY the entity's own name
    /// (the common case: no GM alternate names were ever added) must parse
    /// to zero GM aliases, not one alias equal to the name.
    #[test]
    fn a_frontmatter_aliases_list_with_only_the_name_yields_no_gm_aliases() {
        let record = npc(); // npc()'s aliases is vec![] -> frontmatter aliases == [name]
        let exported = render_record(&record);
        let (fm, _body) = crate::frontmatter::parse(&exported).expect("parse rendered file");
        assert_eq!(fm.aliases, vec!["Seraphina Aldric".to_string()]);
        let recovered = crate::frontmatter::gm_aliases(fm.name.as_deref(), &fm.aliases);
        assert_eq!(
            recovered,
            Vec::<String>::new(),
            "a frontmatter aliases list containing only the entity's own name must yield zero GM aliases"
        );
    }

    #[test]
    fn render_record_fences_the_compiled_article() {
        let out = render_record(&npc());
        assert!(out.contains(crate::markdown::FENCE_START));
        assert!(out.contains("[[The Iron Tower]]"));
        assert!(out.contains(crate::markdown::FENCE_END));
    }

    #[test]
    fn render_record_never_emits_is_gm_only() {
        // The manual flag was built and reverted; GM-secret is Phase 3.
        assert!(!render_record(&npc()).contains("is_gm_only"));
    }

    #[test]
    fn render_record_of_an_entity_with_no_article_omits_the_fence() {
        let VaultRecord::Entity(mut e) = npc() else {
            unreachable!()
        };
        e.codex_article = None;
        let out = render_record(&VaultRecord::Entity(e));
        assert!(!out.contains(crate::markdown::FENCE_START));
    }

    #[test]
    fn render_record_of_a_session_has_no_fence_and_carries_title() {
        let rec = VaultRecord::Session(SessionRecord {
            vref: VaultRef {
                table: "session".into(),
                id: "s1".into(),
            },
            session_number: 1,
            title: "The Awakening".into(),
            date_played: "2026-01-01".into(),
            notes: "Recap.".into(),
            campaign: VaultScope::Campaign {
                id: "campaign:c1".into(),
                name: "SoV".into(),
            },
            created_at: "x".into(),
            updated_at: "y".into(),
        });
        let out = render_record(&rec);
        assert!(
            !out.contains(crate::markdown::FENCE_START),
            "sessions have no compiled body"
        );
        assert!(out.contains(r#"title: "The Awakening""#));
        assert!(out.contains("session_number: 1"));
        assert!(out.contains("Recap."));
    }

    #[test]
    fn content_hash_ignores_trailing_newlines_and_crlf() {
        assert_eq!(content_hash("body"), content_hash("body\n"));
        assert_eq!(content_hash("a\nb"), content_hash("a\r\nb\r\n"));
    }

    #[test]
    fn content_hash_distinguishes_different_content() {
        assert_ne!(content_hash("a"), content_hash("b"));
    }

    #[test]
    fn render_record_is_deterministic() {
        assert_eq!(render_record(&npc()), render_record(&npc()));
    }

    /// Locks the exact byte layout of the frontmatter/body seam. A drive-by
    /// whitespace change here would re-hash every synced file as "changed"
    /// and force a spurious full re-export.
    #[test]
    fn rendered_record_layout_is_stable_at_the_seam() {
        use pretty_assertions::assert_eq;
        let record = VaultRecord::Entity(EntityRecord {
            vref: VaultRef {
                table: "npc".into(),
                id: "n1".into(),
            },
            name: "Seraphina".into(),
            summary: Some("S.".into()),
            notes: Some("N.".into()),
            codex_article: Some("C.".into()),
            aliases: vec![],
            scope: VaultScope::Campaign {
                id: "campaign:c1".into(),
                name: "SoV".into(),
            },
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-02T00:00:00Z".into(),
        });
        let rendered = render_record(&record);
        // Pin the FULL literal: run the test once, paste the actual output
        // here verbatim, and review that it is what the grammar promises
        // (frontmatter fences, one blank line, Summary, fenced article, Notes).
        let expected = "---\n\
             id: \"npc:n1\"\n\
             name: \"Seraphina\"\n\
             title: \"Seraphina\"\n\
             aliases: [\"Seraphina\"]\n\
             type: \"npc\"\n\
             campaign: \"SoV\"\n\
             created_at: \"2026-01-01T00:00:00Z\"\n\
             updated_at: \"2026-01-02T00:00:00Z\"\n\
             ---\n\
             \n\
             ## Summary\n\
             \n\
             S.\n\
             \n\
             <!-- chronacle:codex-article start -- compiled; edits are not applied -->\n\
             C.\n\
             <!-- chronacle:codex-article end -->\n\
             \n\
             ## Notes\n\
             \n\
             N.\n";
        assert_eq!(rendered, expected);
    }
}
