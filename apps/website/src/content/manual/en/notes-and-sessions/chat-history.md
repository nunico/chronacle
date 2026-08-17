---
translationKey: notes.chat-history
locale: en
slug: notes-and-sessions/chat-history
title: Understand chat history
summary: Know which saved messages appear when you switch campaigns and what the current view does without one.
section: notes-and-sessions
order: 3
headings:
  - id: how-history-is-scoped
    text: How history is scoped
    level: 2
  - id: revisit-a-thread
    text: Revisit a thread
    level: 2
  - id: example
    text: Example
    level: 2
  - id: current-limits
    text: Current limits
    level: 2
---

Completed questions and answers are saved in time order, and selecting a campaign loads only messages stored for that campaign.

<h2 id="how-history-is-scoped">How history is scoped</h2>

The question is saved when a request begins; a completed answer is saved when generation finishes. Switching campaigns reloads the thread for the newly selected campaign. With no campaign selected, the current app requests the complete stored message history rather than a separate global-only thread.

<h2 id="revisit-a-thread">Revisit a thread</h2>

1. Choose **Lanterns of Greyharbor** in the campaign rail.
2. Open **Oracle** and scroll upward through its stored exchange.
3. Choose **Jump to latest** when the button appears to return to the newest message.

<h2 id="example">Example</h2>

Ask in **Lanterns of Greyharbor**, “Who promised passage at moonrise?” After the answer completes, switch to **The Brass Orchard**; the Greyharbor exchange disappears. Switch back and it returns in its original order.

<h2 id="current-limits">Current limits</h2>

- There is no clear-history or start-new-thread control in the current Oracle view.
- Cancelling generation keeps a partial answer visible for the moment, but that partial answer is not saved as an assistant message.
- A failed request appears with **Retry**; retry sends the preceding question again.
