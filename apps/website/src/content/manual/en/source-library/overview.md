---
translationKey: sources.overview
locale: en
slug: source-library/overview
title: Source library overview
summary: Organize PDFs in reusable collections and decide which collections each campaign can search.
section: source-library
order: 1
headings:
  - id: how-the-library-works
    text: How the library works
    level: 2
  - id: expected-result
    text: Expected result
    level: 2
  - id: example
    text: Example
    level: 2
  - id: storage-and-online-answers
    text: Storage and online answers
    level: 2
---

<h2 id="how-the-library-works">How the library works</h2>

Put each PDF in a collection, then subscribe campaigns to the collections they should search.

1. Select **Campaign & sources** in the side rail.
2. Create or select a campaign under **Manage campaigns**.
3. Under **Source collections**, use the switch beside each collection to subscribe or unsubscribe the active campaign.
4. Expand a collection to see its **Books** and each book's indexing status.
5. Use **Add book** to import another PDF directly into that collection.
6. Return to **Oracle** with the campaign active; its questions search the subscribed collections.

<h2 id="expected-result">Expected result</h2>

The campaign view shows its collection and book counts, and Oracle answers can draw from books marked **Indexed** in subscribed collections.

<h2 id="example">Example</h2>

Create **Shared Sea Rules** with `Voyages of the Opal Sea.pdf` and **Blackwake Lore** with `Secrets of Blackwake.pdf`. Subscribe **The Blackwake Bell** to both, but subscribe **Isles of Dawn** only to **Shared Sea Rules**. In **The Blackwake Bell**, ask:

> Who keeps the drowned bell beneath Blackwake lighthouse?

The answer can use `[Source: "Secrets of Blackwake.pdf", p.74]`; the **Isles of Dawn** campaign does not search that collection unless you subscribe it too.

<h2 id="storage-and-online-answers">Storage and online answers</h2>

- Chronacle stores the imported PDF and its search index in the desktop app's local data.
- When an online answer provider is active, Chronacle sends it the question and the answer context it supplies. That can include relevant source excerpts; entity names, summaries, notes, and compiled Codex articles; player names and character class, level, and status; event start and end dates; session numbers, titles, played dates, and notes; and compiled rules. Campaign entities and sessions can be full campaign-scoped context rather than relevance-filtered results.
- A collection can serve more than one campaign, so a shared PDF needs to be imported and indexed only once.
- Continue with [collections](/en/manual/source-library/collections), [PDF import](/en/manual/source-library/upload-pdfs), or [indexing](/en/manual/source-library/indexing).
