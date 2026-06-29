use super::rows::{BasicRow, EventRow, PcRow, SessionRow};

/// Max characters of an entity/session note included in the context block.
/// Notes can be long; we include a leading excerpt so the LLM sees the GM's
/// own prose without letting a single entity dominate the prompt budget.
pub(super) const NOTES_EXCERPT_LEN: usize = 280;

/// Format a notes field as a single-line context excerpt, or `None` when empty.
///
/// Newlines are collapsed to spaces so each entity stays on its own line, and
/// the text is truncated on a char boundary with an ellipsis when over budget.
pub(super) fn notes_excerpt(notes: Option<&str>) -> Option<String> {
    let trimmed = notes?.trim();
    if trimmed.is_empty() {
        return None;
    }
    let collapsed = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= NOTES_EXCERPT_LEN {
        Some(collapsed)
    } else {
        let truncated: String = collapsed.chars().take(NOTES_EXCERPT_LEN).collect();
        Some(format!("{truncated}…"))
    }
}

/// Format the fetched entity data into a context string for the LLM prompt.
#[allow(clippy::too_many_arguments)] // one arg per entity table — the shape is fixed by the schema
pub(super) fn format_entity_output(
    pcs: &[PcRow],
    npcs: &[BasicRow],
    locations: &[BasicRow],
    factions: &[BasicRow],
    creatures: &[BasicRow],
    items: &[BasicRow],
    events: &[EventRow],
    misc: &[BasicRow],
    sessions: &[SessionRow],
    col_entities: &[(String, BasicRow)],
) -> String {
    let mut out = String::from("Campaign notes (your GM records):\n");

    if !pcs.is_empty() {
        out.push('\n');
        for r in pcs {
            out.push_str(&format!("[player_character] {}", r.name));
            if let Some(p) = &r.player_name {
                out.push_str(&format!(" · Player: {p}"));
            }
            if let Some(c) = &r.character_class {
                out.push_str(&format!(" · Class: {c}"));
            }
            if let Some(l) = r.character_level {
                out.push_str(&format!(" · Level: {l}"));
            }
            if let Some(s) = &r.status {
                out.push_str(&format!(" · Status: {s}"));
            }
            if let Some(s) = &r.summary {
                if !s.trim().is_empty() {
                    out.push_str(&format!(" · {s}"));
                }
            }
            if let Some(n) = notes_excerpt(r.notes.as_deref()) {
                out.push_str(&format!(" · Notes: {n}"));
            }
            out.push('\n');
        }
    }

    for (rows, kind) in [
        (npcs, "npc"),
        (locations, "location"),
        (factions, "faction"),
        (creatures, "creature"),
        (items, "item"),
    ] {
        if !rows.is_empty() {
            out.push('\n');
            for r in rows {
                out.push_str(&format!("[{kind}] {}", r.name));
                if let Some(s) = &r.summary {
                    if !s.trim().is_empty() {
                        out.push_str(&format!(" · {s}"));
                    }
                }
                if let Some(n) = notes_excerpt(r.notes.as_deref()) {
                    out.push_str(&format!(" · Notes: {n}"));
                }
                out.push('\n');
            }
        }
    }

    if !events.is_empty() {
        out.push('\n');
        for r in events {
            out.push_str(&format!("[event] {}", r.name));
            match (&r.date_start, &r.date_end) {
                (Some(s), Some(e)) if !s.trim().is_empty() && !e.trim().is_empty() => {
                    out.push_str(&format!(" · {s} → {e}"));
                }
                (Some(s), _) if !s.trim().is_empty() => {
                    out.push_str(&format!(" · {s}"));
                }
                _ => {}
            }
            if let Some(s) = &r.summary {
                if !s.trim().is_empty() {
                    out.push_str(&format!(" · {s}"));
                }
            }
            if let Some(n) = notes_excerpt(r.notes.as_deref()) {
                out.push_str(&format!(" · Notes: {n}"));
            }
            out.push('\n');
        }
    }

    if !misc.is_empty() {
        out.push('\n');
        for r in misc {
            out.push_str(&format!("[misc] {}", r.name));
            if let Some(s) = &r.summary {
                if !s.trim().is_empty() {
                    out.push_str(&format!(" · {s}"));
                }
            }
            if let Some(n) = notes_excerpt(r.notes.as_deref()) {
                out.push_str(&format!(" · Notes: {n}"));
            }
            out.push('\n');
        }
    }

    if !sessions.is_empty() {
        out.push('\n');
        for r in sessions {
            match r.session_number {
                Some(num) => out.push_str(&format!("[session {num}] {}", r.title)),
                None => out.push_str(&format!("[session] {}", r.title)),
            }
            if let Some(d) = &r.date_played {
                if !d.trim().is_empty() {
                    out.push_str(&format!(" · {d}"));
                }
            }
            if let Some(n) = notes_excerpt(r.notes.as_deref()) {
                out.push_str(&format!(" · Notes: {n}"));
            }
            out.push('\n');
        }
    }

    // ── Collection entities section ──────────────────────────────────────────
    if !col_entities.is_empty() {
        out.push_str("\nCollection knowledge (from subscribed rulebooks):\n");
        for (kind, r) in col_entities {
            out.push_str(&format!("[{kind}] {}", r.name));
            if let Some(s) = &r.summary {
                if !s.trim().is_empty() {
                    out.push_str(&format!(" · {s}"));
                }
            }
            if let Some(n) = notes_excerpt(r.notes.as_deref()) {
                out.push_str(&format!(" · Notes: {n}"));
            }
            out.push('\n');
        }
    }

    out
}

/// Build a context block from search results for the LLM prompt.
pub fn build_context(results: &[chronacle_core::vector_store::SearchResult]) -> String {
    if results.is_empty() {
        return String::new();
    }
    let mut ctx = String::from("Relevant source material:\n\n");
    for (i, r) in results.iter().enumerate() {
        let source = if r.source_name.is_empty() {
            &r.source_id
        } else {
            &r.source_name
        };
        ctx.push_str(&format!(
            "[{i}] Source: \"{source}\", p. {}-{} — \"{}\"\n{}\n\n",
            r.page_start, r.page_end, r.section_heading, r.text
        ));
    }
    ctx
}
