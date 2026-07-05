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

/// Rule category definitions, including the resource-vs-statistic
/// disambiguation few-shot, shared by the compile and redo prompts.
pub(super) const RULE_CATEGORY_DEFS: &str = "Rule categories (choose the single best fit):
- mechanic: a discrete rule or subsystem (initiative, opposed checks, downtime).
- ability: a named capability an actor can use (spell, feat, technique, power, maneuver).
- state: a condition or status affecting an actor (poisoned, exhausted, hunted).
- procedure: a step-by-step sequence (character creation, long rest, chase scene).
- resource: a countable in-play thing that is spent or restored during play (hit points, mana, stress, ammo).
- statistic: a numerical value used or modified in or by another rule (armor class, movement speed, carrying capacity). NOTE: hit points are a resource (spent/restored); armor class is a statistic (referenced/modified) — do not confuse the two.
- entry: freeform fallback when nothing above fits.";

/// Build the prompt for one rules-compile batch of labeled chunks.
///
/// `labeled_chunks` is pre-labeled: each passage prefixed with
/// `[Source: "<name>", p.X-Y]`.
pub(super) fn build_rules_prompt(labeled_chunks: &str) -> String {
    format!(
        r#"You are extracting discrete, reusable RULES from TTRPG rulebook passages.

Extract every distinct rule, mechanic, ability, condition, procedure, resource, or statistic
described in the passages below. Skip pure lore, flavor text, or narrative color that contains
no actionable rule.

For each rule you find, write a self-contained rule entry: a name, a category, and body text
that fully explains the rule without requiring the reader to consult the source.

{RULE_CATEGORY_DEFS}

Cite every page you drew the rule from in `page_refs`, using the exact source name and page
numbers shown in the passage labels.

Return ONLY JSON, no prose, no markdown fences, matching exactly this shape:
{{ "entries": [ {{ "name": "…", "category": "mechanic|ability|state|procedure|resource|statistic|entry",
                 "body": "self-contained rule text",
                 "page_refs": [ {{ "source_name": "…", "page_start": 1, "page_end": 2 }} ] }} ] }}

Source passages:
{labeled_chunks}"#
    )
}

/// Build the prompt to regenerate ONE rule entry honoring every GM objection.
pub(super) fn build_rules_redo_prompt(
    entry_name: &str,
    current_body: &str,
    objections: &[String],
    labeled_chunks: &str,
) -> String {
    let objections_block = if objections.is_empty() {
        String::from("(none)")
    } else {
        objections
            .iter()
            .enumerate()
            .map(|(i, o)| format!("{}. {}", i + 1, o))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        r#"You are revising ONE rules-codex entry, "{entry_name}", based on GM objections.

Current body:
{current_body}

You MUST honor every one of these GM objections in the revised entry (all of them, not just the
most recent):
{objections_block}

{RULE_CATEGORY_DEFS}

Return ONLY JSON, no prose, no markdown fences, matching exactly this shape (a single entry):
{{ "entries": [ {{ "name": "{entry_name}", "category": "mechanic|ability|state|procedure|resource|statistic|entry",
                 "body": "revised self-contained rule text",
                 "page_refs": [ {{ "source_name": "…", "page_start": 1, "page_end": 2 }} ] }} ] }}

Source passages:
{labeled_chunks}"#
    )
}

/// Build the prompt that distills a chat answer into targeted codex proposals.
///
/// `known_entities` are the in-scope entity names (with kinds) so the LLM can
/// target existing records; anything else becomes a `new_entity` draft.
pub(super) fn build_chat_distill_prompt(answer: &str, known_entities: &str) -> String {
    format!(
        r#"You are maintaining a TTRPG campaign codex. A cited answer was just given to the GM.
Distill it into zero or more SMALL, TARGETED update proposals for the codex. Only propose changes
that add durable knowledge — skip restatements of what the codex already implies, greetings, or
speculation. Never invent facts not present in the answer.

Proposal kinds:
- entity_article_update: improve an existing entity's compiled article (target an entity below).
- entity_notes_update: suggest an addition to the GM's own notes on an entity (rare; only for
  table-decision-like facts).
- new_entity: a person/place/faction/creature/item/event named in the answer but missing below.
  Set entity_kind to one of: npc, location, faction, creature, item, event, misc.
- rule_entry_update / new_rule_entry: only for rules content, with category one of:
  mechanic, ability, state, procedure, resource, statistic, entry.

Known entities (name — kind):
{known_entities}

Return ONLY JSON, no prose, no markdown fences:
{{ "proposals": [ {{ "kind": "…", "target_name": "…", "entity_kind": null, "category": null,
                   "proposed_text": "…", "rationale": "…" }} ] }}

The answer:
{answer}"#
    )
}

/// Build the prompt that distills saved session notes into proposals and a
/// mentioned-entity list (used to mark staleness).
pub(super) fn build_session_distill_prompt(notes: &str, known_entities: &str) -> String {
    format!(
        r#"You are maintaining a TTRPG campaign codex. The GM just saved session notes.
Extract durable knowledge: propose entity article updates for entities whose story moved, and
new_entity drafts for people/places/things that appear in the notes but not in the known list.
Also list EVERY known entity mentioned in the notes (exact names from the list).

Known entities (name — kind):
{known_entities}

Return ONLY JSON, no prose, no markdown fences:
{{ "proposals": [ {{ "kind": "entity_article_update|new_entity", "target_name": "…",
                   "entity_kind": null, "proposed_text": "…", "rationale": "…" }} ],
  "mentioned": [ "…" ] }}

Session notes:
{notes}"#
    )
}
