---
translationKey: providers.choose
locale: en
slug: ai-providers/choose
title: Choose an AI provider
summary: Pick the service that writes answers, then choose a separate search-index mode for your sources.
section: ai-providers
order: 1
headings:
  - id: make-two-choices
    text: Make two choices
    level: 2
  - id: expected-result
    text: Expected result
    level: 2
  - id: example
    text: Example
    level: 2
  - id: decision-guide
    text: Decision guide
    level: 2
---

<h2 id="make-two-choices">Make two choices</h2>

Choose one provider to write answers and one indexing mode to help Chronacle find relevant source passages.

1. Open **Settings** and find **LLM provider**. An LLM provider is the service or local program that turns your question and the answer context Chronacle supplies into a reply.
2. Choose **OpenAI** or **Anthropic** for the standard [online setup](/en/manual/ai-providers/online), **Ollama (local)** for a [local answer model](/en/manual/ai-providers/local), or a registered [custom provider](/en/manual/ai-providers/custom).
3. Enter the exact **Model** identifier expected by that provider.
4. Choose **Save & connect** and look for **Connected: …**
5. In **Embedding provider**, choose how Chronacle builds its search index. This setting is independent of the answer provider.
6. If you change the indexing model after importing sources, use **Re-index all sources**.

<h2 id="expected-result">Expected result</h2>

**Connection status** shows the active answer provider and model, while **Embedding provider** separately shows the active indexing model and dimension.

<h2 id="example">Example</h2>

For the **Saffron Reaches** campaign, choose Anthropic as the answer provider and **Multilingual local — E5 Base (offline)** for a German source named `Die Salzkrone.pdf`. After connecting, importing, and indexing it, ask:

> Wem schuldet Kapitänin Vael noch einen Gefallen?

The reply can be written in German and cite `[Source: "Die Salzkrone.pdf", p.67]` when the passage is retrieved.

<h2 id="decision-guide">Decision guide</h2>

- Choose an online provider when you want its supported models and can provide the required credentials.
- Choose Ollama only if you already want to run and maintain a local model; local speed and answer quality depend on the model and computer.
- Choose multilingual indexing for German, French, Spanish, or questions in a different language from the source.
- When an online answer provider is active, Chronacle sends it the question and the answer context Chronacle supplies. Depending on the selected campaign and sources, that can include relevant source excerpts, entity names, summaries, notes and compiled Codex articles, session titles and notes, and compiled rules. Campaign entities and sessions can be included as full campaign-scoped context rather than relevance-filtered results.
