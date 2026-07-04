//! Prompt construction for codex article compilation.

/// Build the article-compilation prompt for one entity.
///
/// The LLM must ground every statement in the supplied passages and cite with
/// inline `[Source: "<name>", p.N]` markers; in-world entity names from the
/// neighbor list become `[[wikilinks]]`.
pub(super) fn build_article_prompt(
    name: &str,
    kind: &str,
    summary: Option<&str>,
    notes: Option<&str>,
    neighbors: &[(String, String)], // (name, rel_type)
    passages: &str, // pre-labeled: each passage prefixed with [Source: "...", p.X-Y]
) -> String {
    let neighbor_block = if neighbors.is_empty() {
        String::from("(none)")
    } else {
        neighbors
            .iter()
            .map(|(n, r)| format!("- [[{n}]] ({r})"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        r#"You are compiling the reference article for a TTRPG campaign codex.

Write the definitive article about the {kind} "{name}".

Rules:
- Use ONLY facts present in the source passages, the summary, or the notes below. NEVER invent facts.
- Cite every claim taken from a passage with its inline marker, exactly as given: [Source: "<name>", p.N]
- When you mention one of the related entities listed below, write its name as a [[wikilink]].
- Write flowing prose (2-6 paragraphs). No headings, no bullet lists, no preamble — start directly with the article text.

Known summary: {summary}
GM notes: {notes}
Related entities:
{neighbor_block}

Source passages:
{passages}"#,
        summary = summary.unwrap_or("(none)"),
        notes = notes.unwrap_or("(none)"),
    )
}
