---
translationKey: settings.overview
locale: en
slug: settings/overview
title: Use Settings safely
navTitle: Settings overview
summary: Configure language, answer and search providers, maintenance actions, extraction, and the Markdown vault.
section: settings
order: 1
headings:
  - id: choose-language-and-answer-provider
    text: Choose language and answer provider
    level: 2
  - id: choose-search-provider
    text: Choose search provider
    level: 2
  - id: maintenance-and-vault
    text: Maintenance and vault
    level: 2
  - id: example
    text: Example
    level: 2
  - id: safe-practice
    text: Safe practice
    level: 2
---

Settings control how Chronacle speaks, answers, finds relevant passages, maintains links, and mirrors Markdown files. Use each section’s own save or action button; changing a field alone does not necessarily activate it.

<h2 id="choose-language-and-answer-provider">Choose language and answer provider</h2>

**Display language** saves immediately. If that save fails, Chronacle restores the last saved language and shows an error. **Automatic** follows the supported system language and otherwise uses English.

Under **LLM provider**, choose OpenAI, Anthropic, Ollama (local), or a registered custom provider. This is the service that writes answers from the context Chronacle supplies. **Save Settings** attempts to store Provider, API key, Model, and any visible Base URL. **Save & Connect** runs the same save attempt, then replaces the active provider from the stored values without an app restart. If `Failed to save: {error}` appears, do not assume your edited values became active even if `Connected: {provider}` follows; correct the save error and try again. An activation failure appears as `Connection failed: {error}`.

OpenAI, Anthropic, and custom providers require an API key in this connection form; Ollama does not. A non-empty Base URL must be a valid URL. For a built-in provider, a blank Model uses Chronacle’s hard-coded compatibility default, not a default selected by the provider. For a custom provider, choose a model you registered and that the endpoint supports. Start with [Choose an AI provider](/en/manual/ai-providers/choose) or [Set up a custom provider](/en/manual/ai-providers/custom).

<h2 id="choose-search-provider">Choose search provider</h2>

**Embedding provider** controls how document and question text is prepared for relevance search. Choose **Small local — Nomic (offline)**, **Multilingual local — E5 Base (offline)**, or **Cloud — OpenAI-compatible API**. Cloud mode requires credentials and a model that produces 768 values; its Base URL is optional.

With cloud embedding, the configured remote embedding endpoint receives each piece of searchable text that Chronacle prepares, as applicable: source chunks; entity names, summaries, notes, and compiled Codex articles; session titles and notes; compiled rules; and question or search text during retrieval. These categories are sent when Chronacle needs to embed them, not all together with every request. Local embedding performs this calculation on this computer. This does not make the separate answer provider local: when answering, that provider receives the question and the excerpts Chronacle retrieved.

**Save embedding provider** stores all four embedding fields and activates the provider without restarting. An undownloaded local model leaves the previous provider active and reveals **Download selected model**. After a successful provider or model change, use **Re-index all sources** so existing PDFs use it too. Chronacle deletes one source’s old passages before rebuilding it, with no rollback. That source is unavailable in search during the attempt and remains unavailable after a failed attempt until a retry succeeds; other sources remain available.

<h2 id="maintenance-and-vault">Maintenance and vault</h2>

- **Custom providers** registers compatible services and their model IDs. Use the exact ID supplied by that service.
- **Rebuild relationship links** re-reads `[[links]]` in notes. It is useful after importing notes or for older entities.
- **Enrich related entities** saves as soon as you toggle it. It adds a slower second extraction pass and more answer-provider calls, capped at 20 related entities per extraction.
- **Markdown vault** connects a local folder, checks it, lists conflicts, and offers **Sync now**. Read [Keep a Markdown vault](/en/manual/vault/overview) before choosing a populated folder.

<h2 id="example">Example</h2>

Your Valdris campaign uses German notes. Set **Display language** to German, choose **Multilingual local — E5 Base (offline)**, download it if prompted, save it, then run **Re-index all sources**. Ask “Was weiß Mara Venn über den Iron Tower?” after re-indexing; Chronacle can now match the German question against the campaign material using the selected search model.

<h2 id="safe-practice">Safe practice</h2>

- Never paste an API key into notes, chat, screenshots, or support messages. Enter it only in the password-style settings field.
- **Save Settings** and **Save & Connect** have different results; use the latter when you want the answer provider active now.
- Copy the exact displayed error before changing several fields. The error may show that a save, connection, download, or re-index step failed, but it does not prove a deeper cause.
- A provider switch does not require a restart. A search-model switch does require re-indexing existing sources for consistent results.
