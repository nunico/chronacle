---
translationKey: providers.online
locale: en
slug: ai-providers/online
title: Connect an online provider
summary: Connect OpenAI or Anthropic with your API key and the exact model identifier you want to use.
section: ai-providers
order: 2
headings:
  - id: connect-the-provider
    text: Connect the provider
    level: 2
  - id: expected-result
    text: Expected result
    level: 2
  - id: example
    text: Example
    level: 2
  - id: important-details
    text: Important details
    level: 2
---

<h2 id="connect-the-provider">Connect the provider</h2>

Enter your provider credentials and model in **Settings**, then let Chronacle test the configuration.

1. Obtain an API key from OpenAI or Anthropic using that provider's own instructions.
2. In Chronacle, open **Settings** and find **LLM provider**.
3. Under **Provider**, choose **OpenAI** or **Anthropic**.
4. Paste the key into **API key**.
5. Enter an exact **Model** identifier supported for your provider access.
6. Choose **Save & connect**.
7. Confirm that the banner says **Connected: openai** or **Connected: anthropic**, and check the values under **Connection status**.

<h2 id="expected-result">Expected result</h2>

Chronacle reports a successful connection and uses the selected provider for subsequent Oracle replies.

<h2 id="example">Example</h2>

Connect Anthropic, select a model available to you, subscribe **The Brass Observatory** campaign to the **Sky Charts** collection, and ask:

> Why does the astronomer Nera cover the eastern lens at midnight?

Chronacle sends the question and the answer context it supplies to the provider, then can return an answer with `[Source: "Observatory Notes.pdf", p.23]`. That context can include relevant source excerpts, entity names, summaries, notes and compiled Codex articles, session titles and notes, and compiled rules. Campaign entities and sessions can be supplied as full campaign-scoped context rather than relevance-filtered results.

<h2 id="important-details">Important details</h2>

- Provider availability, model access, and charges are set by the provider; check its current documentation.
- **Save settings** stores the form. **Save & connect** also applies and tests the connection now.
- The **Embedding provider** section controls search indexing separately. Your online answer provider does not automatically become the indexing provider.
- A connection error reports the failure Chronacle received. Check the key, exact model ID, and any displayed base URL before retrying.
