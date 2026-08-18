---
translationKey: providers.language-search
locale: en
slug: ai-providers/language-and-search
title: Language and search
summary: Match the interface and reply language to your table, then choose an index suited to your source languages.
section: ai-providers
order: 5
headings:
  - id: set-language-and-indexing
    text: Set language and indexing
    level: 2
  - id: expected-result
    text: Expected result
    level: 2
  - id: example
    text: Example
    level: 2
  - id: what-each-setting-changes
    text: What each setting changes
    level: 2
---

<h2 id="set-language-and-indexing">Set language and indexing</h2>

Set the display language for the controls, then choose an indexing model that can retrieve the languages used in your PDFs and questions.

1. In **Settings**, set **Display language** to **Automatic**, **English**, **Deutsch**, **Français**, or **Español**. The interface changes immediately.
2. Under **Embedding mode**, choose **Small local — Nomic (offline)** for English-focused retrieval, or **Multilingual local — E5 Base (offline)** for German, French, Spanish, and cross-language retrieval.
3. If you use **Cloud — OpenAI-compatible API**, enter credentials and a model that returns 768 dimensions, as required by the settings screen.
4. Choose **Save embedding provider**. If Chronacle asks for the selected local model, choose **Download selected model**.
5. Choose **Re-index all sources** so existing PDFs use the new model.

<h2 id="expected-result">Expected result</h2>

The interface uses your selected language, new questions receive a reply in a clearly detected supported question language, and ambiguous questions fall back to the interface language.

<h2 id="example">Example</h2>

Set the interface to **Deutsch**, choose **Multilingual local — E5 Base (offline)**, re-index the English `Atlas of Quiet Stars.pdf`, and ask:

> Warum meidet Serin den Nordturm?

Chronacle can retrieve the English passage, answer in German, and cite `[Source: "Atlas of Quiet Stars.pdf", p.36]`. The source name and quoted text remain as written in the PDF.

<h2 id="what-each-setting-changes">What each setting changes</h2>

- **Display language** changes Chronacle's controls and is the fallback reply language for short or ambiguous questions.
- **Embedding mode** changes how source text and questions are indexed for search. Changing it requires re-indexing existing sources.
- **LLM provider** changes who writes the answer. When it is online, it receives the question and the answer context Chronacle supplies. That can include relevant source excerpts; entity names, summaries, notes, and compiled Codex articles; player names and character class, level, and status; event start and end dates; session numbers, titles, played dates, and notes; and compiled rules. Campaign entities and sessions can be full campaign-scoped context rather than relevance-filtered results.
- A multilingual index improves retrieval across supported languages; it does not translate the stored PDF or rewrite its names.
