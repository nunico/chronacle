---
translationKey: glossary.main
locale: en
slug: glossary
title: Glossary
summary: Plain-language meanings for the words Chronacle uses around sources, campaigns, Codex, and vault files.
section: glossary
order: 1
headings:
  - id: sources-and-answers
    text: Sources and answers
    level: 2
  - id: campaigns-and-codex
    text: Campaigns and Codex
    level: 2
  - id: vault-terms
    text: Vault terms
    level: 2
  - id: example
    text: Example
    level: 2
---

Use these meanings when a Chronacle screen or this manual uses an unfamiliar term.

<h2 id="sources-and-answers">Sources and answers</h2>

**Source.** One imported PDF and its processing status. See [Use the source library](/en/manual/source-library/overview).

**Collection.** A named group of sources that can be attached to more than one campaign. See [Organise collections](/en/manual/source-library/collections).

**Campaign.** Your playable workspace: its own entities, sessions, chat history, and access to selected source collections. See [Campaigns and their boundaries](/en/manual/campaigns/overview).

**Chunk or passage.** A small piece of text cut from a source or note so Chronacle can find the relevant part instead of sending an entire book with every question.

**Embedding.** A numeric representation of searchable text. As applicable, Chronacle creates these for source chunks; entity names, summaries, notes, and compiled Codex articles; session titles and notes; compiled rules; and question or search text so it can compare relevant material.

**Index.** The prepared, searchable passages made during PDF processing. Re-indexing rebuilds them from the saved source. See [Understand indexing](/en/manual/source-library/indexing).

**Answer provider.** The configured AI service that writes the final answer from the passages and instructions Chronacle supplies. See [Choose an AI provider](/en/manual/ai-providers/choose).

**Citation.** A link on an answer that identifies the source and page behind a claim. See [Check citations](/en/manual/notes-and-sessions/citations).

<h2 id="campaigns-and-codex">Campaigns and Codex</h2>

**Codex Article.** Generated reference prose for an entity. It can be replaced by compilation, unlike your Summary and Notes. See [Separate articles from your notes](/en/manual/codex/articles-and-notes).

**Table notes.** Your own lasting comments on a compiled rule, preserved when its generated body is updated.

**Finding.** A maintenance item that points out a possible name, link, type, or compiled-content problem for you to review. It is not automatically a confirmed mistake. See [Maintain the Codex](/en/manual/codex/maintenance).

**Alias or alternate name.** Another name that identifies the same entity or rule, such as “The Lantern” for Mara Venn. See [Add alternate names](/en/manual/vault/alternate-names).

**Session.** A numbered play record with a title, played date, notes, and linked events. See [Keep a session log](/en/manual/notes-and-sessions/session-log).

<h2 id="vault-terms">Vault terms</h2>

**Vault.** A local folder where Chronacle mirrors supported records as Markdown files and can apply supported edits back. See [Keep a Markdown vault](/en/manual/vault/overview).

**Conflict.** A record state created when both Chronacle and its Markdown file changed differently after their last shared version. Both versions are preserved while the record is frozen. See [Resolve vault conflicts](/en/manual/vault/conflicts).

<h2 id="example">Example</h2>

You import the source _Harbourmaster’s Field Notes_ into the **Valdris References** collection and attach it to the **Shadows of Valdris** campaign. Chronacle splits the source into passages and indexes them. The answer provider uses matching passages to answer “Why does Mara Venn avoid North Quay?” and returns a citation. You save Mara as an entity, keep table facts in Notes, compile a Codex Article, add “The Lantern” as an alias, and record the discovery in session 012. If Mara’s Chronacle note and vault file later change separately, that is a conflict.
