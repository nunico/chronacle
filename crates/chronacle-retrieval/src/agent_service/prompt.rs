//! System-prompt assembly for the GM assistant, adapting to which of the RAG
//! and campaign-notes contexts are present.

/// Build the system prompt for the GM assistant.
///
/// Accepts separate RAG context (retrieved source passages), entity context
/// (campaign notes), and rules context (compiled rule entries). Any or all may
/// be empty — the prompt adapts to include only the relevant sections. When
/// present, the RULES block leads, followed by CAMPAIGN NOTES, followed by
/// REFERENCE MATERIAL.
pub(super) fn build_system_prompt(
    rag_context: &str,
    entity_context: &str,
    rules_context: &str,
) -> String {
    let has_rag = !rag_context.is_empty();
    let has_entities = !entity_context.is_empty();
    let has_rules = !rules_context.is_empty();

    if !has_rag && !has_entities && !has_rules {
        return "You are an expert Game Master assistant. \
            Answer the user's question to the best of your ability. \
            If you don't know the answer, say so — do not make up rules."
            .to_string();
    }

    let mut prompt = String::from("You are an expert Game Master assistant.\n\n");

    if has_rules {
        prompt.push_str(&format!("{rules_context}\n"));
    }

    if has_entities {
        prompt.push_str(&format!(
            "CAMPAIGN NOTES (GM's own records):\n{entity_context}\n"
        ));
    }

    if has_rag {
        prompt.push_str(&format!("REFERENCE MATERIAL:\n{rag_context}\n"));
    }

    prompt.push_str("INSTRUCTIONS:\n");
    prompt.push_str("- Read every passage and note above carefully BEFORE answering.\n");

    if has_rag {
        prompt.push_str(
            "- Entity scope is critical. A passage is valid evidence ONLY when it \
             explicitly attributes a fact to the SAME entity the question is about. \
             A passage that lists the target entity alongside OTHER entities \
             (e.g. \"X dominates Vethara, Korim, Suthen and Marrowen\", or \
             \"in Vethara and in Korim\") does NOT attribute everything in the \
             list to the target — those are SEPARATE entities. Wording can vary \
             (synonyms and paraphrases are fine when they refer to the same entity, \
             e.g. \"factions\" ≈ \"groups\"), but a fact about a different but \
             co-mentioned entity is NOT evidence for the target.\n\
             - For list / enumeration questions (\"which are the...\", \"what are the...\", \
             \"list...\"), enumerate EVERY item the passages explicitly attribute to \
             the target entity. Do not compress to fit a sentence budget. If the \
             passages only cover some items, list those and acknowledge that the \
             reference material may be incomplete.\n",
        );
    }

    prompt.push_str(
        "- For other questions, answer in 1–3 sentences. Be concise — the GM is \
         running a table.\n",
    );
    if has_rag {
        prompt.push_str(
            "- Do NOT quote the passages verbatim in your answer text — the supporting \
             quote belongs INSIDE the citation marker.\n",
        );
    }

    if has_rag {
        prompt.push_str(
            "- Every factual claim from REFERENCE MATERIAL must cite its source using \
             this exact format, including a short verbatim quote (1 sentence) from the \
             passage that supports the claim:\n  \
               [Source: \"<source name>\", p.<page>, quote: \"<verbatim sentence>\"]\n  \
             Use the singular key `quote:` with exactly ONE sentence — never \
             `quotes:` or multiple excerpts. Emit a separate marker per source.\n  \
             Example: [Source: \"PHB\", p.72, quote: \"A fighter can use Action Surge once per rest.\"]\n  \
             The UI hides the quote from the visible reply and shows it in a popover \
             when the user clicks the citation badge.\n\
             - Only say \"the reference material does not contain this information\" if you \
             have scanned every passage and found no relevant content (paraphrase counts \
             only for the same entity).\n",
        );
    }

    if has_rules {
        prompt.push_str(
            "- Claims taken from COMPILED RULES cite the book and page shown on the entry, \
             using the same [Source: \"<source name>\", p.<page>, quote: \"<verbatim sentence>\"] \
             format; quote the sentence from the entry body that supports the claim. \
             Lines labeled \"GM table ruling\" are the GM's own house rulings — prefer them \
             over book text when they conflict, and attribute them as the GM's ruling.\n",
        );
    }

    if has_entities {
        prompt.push_str(
            "- Every factual claim from CAMPAIGN NOTES must cite the entity using \
             this exact format:\n  \
               [Entity: \"<entity name>\", kind: \"<kind>\"]\n  \
             where kind is the bracketed prefix in the campaign note line \
             (e.g. player_character, npc, location, faction, creature, item, event, misc).\n  \
             Example: [Entity: \"Nazirdijan\", kind: \"player_character\"]\n  \
             No verbatim quote is needed — entity records are the GM's own data.\n\
             - Entity names in CAMPAIGN NOTES are exact — use them verbatim in citations.\n",
        );
    }

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_prompt_without_context() {
        let prompt = build_system_prompt("", "", "");
        assert!(prompt.contains("Game Master assistant"));
        assert!(!prompt.contains("REFERENCE MATERIAL"));
    }

    #[test]
    fn test_system_prompt_with_context() {
        let ctx =
            "[0] Source: \"PHB.pdf\", p. 72 — \"Fighter Class Features\"\nAction Surge text.\n\n";
        let prompt = build_system_prompt(ctx, "", "");
        assert!(prompt.contains("REFERENCE MATERIAL"));
        assert!(prompt.contains("PHB.pdf"));
        assert!(prompt.contains("[Source: \"<source name>\""));
        assert!(prompt.contains("Do NOT quote the passages"));
        assert!(prompt.contains("1–3 sentences"));
        assert!(prompt.contains("synonyms"));
        assert!(prompt.contains("scanned every passage"));
        assert!(prompt.contains("quote: \""));
    }

    /// Regression for a cross-entity-contamination bug observed in production:
    /// the LLM listed sibling regions as part of the target continent because
    /// the prompt told it to treat "paraphrases and partial matches" as
    /// evidence, with no rule about preserving entity scope.
    ///
    /// The new prompt must (a) explicitly call out the "X dominates A, B, C and
    /// D" trap, (b) require enumeration questions to list every attributed item.
    #[test]
    fn test_system_prompt_guards_entity_scope_and_enumeration() {
        let prompt = build_system_prompt("[0] Source: \"x.pdf\", p. 1 — \"\"\ntext\n\n", "", "");
        // Entity-scope rule must be present.
        assert!(
            prompt.contains("Entity scope is critical"),
            "prompt should warn about cross-entity contamination"
        );
        // The specific failure shape we observed in production.
        assert!(
            prompt.contains("SEPARATE entities"),
            "prompt should explicitly say co-listed entities are SEPARATE"
        );
        // Enumeration questions must not be compressed to 1–3 sentences.
        assert!(
            prompt.contains("enumeration questions") || prompt.contains("list / enumeration"),
            "prompt should call out list / enumeration questions"
        );
        assert!(
            prompt.contains("Do not compress"),
            "prompt should forbid compressing lists into the 1-3 sentence budget"
        );
    }

    #[test]
    fn build_system_prompt_both_contexts_includes_both_sections() {
        let rag = "[0] Source: \"PHB\", p.1 — \"Intro\"\nSome rules.\n\n";
        let ent = "Campaign notes (your GM records):\n\n[npc] Aldric\n";
        let prompt = build_system_prompt(rag, ent, "");
        assert!(prompt.contains("REFERENCE MATERIAL"), "missing RAG section");
        assert!(prompt.contains("CAMPAIGN NOTES"), "missing entity section");
        assert!(
            prompt.contains("[Entity:"),
            "missing entity citation instruction"
        );
        assert!(
            prompt.contains("[Source:"),
            "missing source citation instruction"
        );
    }

    #[test]
    fn build_system_prompt_entity_only_omits_rag_section() {
        let ent = "Campaign notes (your GM records):\n\n[npc] Aldric\n";
        let prompt = build_system_prompt("", ent, "");
        assert!(prompt.contains("CAMPAIGN NOTES"), "missing entity section");
        assert!(
            !prompt.contains("REFERENCE MATERIAL"),
            "unexpected RAG section"
        );
        assert!(
            prompt.contains("[Entity:"),
            "missing entity citation instruction"
        );
        assert!(
            !prompt.contains("Entity scope is critical"),
            "unexpected RAG-only instruction"
        );
    }

    #[test]
    fn build_system_prompt_rag_only_regression() {
        // Regression: existing behaviour must be preserved when entity_context is empty.
        let rag = "[0] Source: \"PHB\", p.1 — \"Intro\"\nSome rules.\n\n";
        let prompt = build_system_prompt(rag, "", "");
        assert!(prompt.contains("REFERENCE MATERIAL"), "missing RAG section");
        assert!(
            !prompt.contains("CAMPAIGN NOTES"),
            "unexpected entity section"
        );
        assert!(
            prompt.contains("Entity scope is critical"),
            "missing scope guard"
        );
        assert!(
            prompt.contains("SEPARATE entities"),
            "missing entity contamination guard"
        );
        assert!(
            prompt.contains("list / enumeration"),
            "missing enumeration instruction"
        );
        assert!(
            prompt.contains("Do not compress"),
            "missing list-compression guard"
        );
    }

    #[test]
    fn build_system_prompt_neither_returns_fallback() {
        let prompt = build_system_prompt("", "", "");
        assert!(
            !prompt.contains("REFERENCE MATERIAL"),
            "unexpected RAG section"
        );
        assert!(
            !prompt.contains("CAMPAIGN NOTES"),
            "unexpected entity section"
        );
        assert!(
            prompt.contains("Game Master assistant"),
            "missing base identity"
        );
    }

    #[test]
    fn rules_block_leads_and_carries_citation_instruction() {
        let rules = "COMPILED RULES (distilled from your rulebooks):\n\n[mechanic] Initiative — PHB p.14\nRoll d20.\n\n";
        let rag = "[0] Source: \"PHB\", p.1 — \"Intro\"\nSome rules.\n\n";
        let ent = "Campaign notes (your GM records):\n\n[npc] Aldric\n";
        let prompt = build_system_prompt(rag, ent, rules);
        let i_rules = prompt.find("COMPILED RULES").expect("rules section");
        let i_notes = prompt.find("CAMPAIGN NOTES").expect("notes section");
        let i_rag = prompt.find("REFERENCE MATERIAL").expect("rag section");
        assert!(
            i_rules < i_notes && i_notes < i_rag,
            "block order must be RULES → CAMPAIGN NOTES → REFERENCE MATERIAL"
        );
        assert!(
            prompt.contains("COMPILED RULES cite the book and page"),
            "rules claims must carry a citation instruction"
        );
    }

    #[test]
    fn no_rules_context_is_todays_behavior() {
        let rag = "[0] Source: \"PHB\", p.1 — \"Intro\"\nSome rules.\n\n";
        let with = build_system_prompt(rag, "", "");
        assert!(!with.contains("COMPILED RULES"));
        assert!(
            with.contains("REFERENCE MATERIAL"),
            "regression: rag-only unchanged"
        );
    }
}
