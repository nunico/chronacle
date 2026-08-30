---
translationKey: sources.collections
locale: en
slug: source-library/collections
title: Organize source collections
summary: Group books into shared shelves and subscribe each campaign to the shelves it needs.
section: source-library
order: 2
headings:
  - id: create-and-subscribe
    text: Create and subscribe
    level: 2
  - id: expected-result
    text: Expected result
    level: 2
  - id: example
    text: Example
    level: 2
  - id: collection-tips
    text: Collection tips
    level: 2
---

<h2 id="create-and-subscribe">Create and subscribe</h2>

Create a collection while importing a PDF, then use campaign subscriptions to control where that collection is searched.

1. Select **Upload PDF** and choose one PDF.
2. In **Add “filename” to collection**, choose **Create new collection**.
3. Enter a **New collection name**, choose **Create**, then select **Upload**.
4. Open **Campaign & sources** and select the campaign you want to configure.
5. Under **Source collections**, turn on the switch beside the new collection.
6. Expand it to see its **Books**, then use **Add book** for more PDFs that belong on the same shelf.

<h2 id="expected-result">Expected result</h2>

The collection appears for every campaign, while the **subscribed** or **not subscribed** state can differ for each campaign.

<h2 id="example">Example</h2>

Create **Wyrdwood Gazetteer** with `Paths Through Wyrdwood.pdf`. Subscribe **The Rowan Crown** to it, but leave **Ashes of Merrow** unsubscribed. Ask in **The Rowan Crown**:

> Which ferryman knows the path to Saint Orra's well?

Chronacle can cite `[Source: "Paths Through Wyrdwood.pdf", p.29]`. The other campaign will not search that collection unless you turn its switch on there.

<h2 id="collection-tips">Collection tips</h2>

- Group books by material you genuinely want to search together, such as shared rules, a setting, or one campaign's specific prep.
- Collections are shared objects; subscriptions are set separately for each campaign.
- The current campaign view shows collection subscription, books, rules, and indexing state. It does not show rename or delete controls for collections.
- Import and indexing details are covered in [Upload PDFs](/en/manual/source-library/upload-pdfs) and [Understand indexing](/en/manual/source-library/indexing).
