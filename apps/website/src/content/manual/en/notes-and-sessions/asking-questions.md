---
translationKey: questions.ask
locale: en
slug: notes-and-sessions/asking-questions
title: Ask a grounded question
summary: Ask in plain language after selecting the campaign and sources that should inform the answer.
section: notes-and-sessions
order: 4
headings:
  - id: ask-step-by-step
    text: Ask step by step
    level: 2
  - id: what-chronacle-uses
    text: What Chronacle uses
    level: 2
  - id: example
    text: Example
    level: 2
  - id: stops-and-errors
    text: Stops and errors
    level: 2
---

Select a campaign, check its source subscriptions, then ask one specific question in the Oracle.

<h2 id="ask-step-by-step">Ask step by step</h2>

1. Choose the campaign in the left rail.
2. In **Campaign & sources**, make sure the relevant collections are **subscribed** and their books are **Indexed**.
3. Open **Oracle** and type into **Ask a rule, a name, a place…**.
4. Press Enter or choose **Send**. Use Shift+Enter for a new line.

<h2 id="what-chronacle-uses">What Chronacle uses</h2>

Chronacle looks for relevant passages in the subscribed collections and can also use saved campaign entities, session notes, and compiled rules. It sends the question and the answer context it supplies to the selected AI provider. That context can include relevant source excerpts, entity names, summaries, notes and compiled Codex articles, session titles and notes, and compiled rules. Campaign entities and sessions are included as full campaign-scoped context rather than relevance-filtered results. With no campaign selected, the current search has no subscribed collection, entity, session, or compiled-rule context.

<h2 id="example">Example</h2>

In **Lanterns of Greyharbor**, ask:

> After the North Quay fire, what did Mara Venn promise, and what does the crossing procedure require before departure?

Chronacle can combine Mara's saved note, the session recap, and the relevant procedure, with source badges for claims drawn from a PDF.

<h2 id="stops-and-errors">Stops and errors</h2>

- Choose **Stop generating** to halt the current answer. Any partial text stays visible but is not saved as a completed assistant message.
- A failed request appears as **The oracle could not answer.** with **Retry** and the error detail.
- A fluent answer can still be wrong; verify important claims through its citations and your notes.
