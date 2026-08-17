---
translationKey: campaigns.sources
locale: en
slug: campaigns/source-access
title: Choose campaign source access
summary: Subscribe a campaign to only the shared collections it should search.
section: campaigns
order: 3
headings:
  - id: set-access
    text: Set access
    level: 2
  - id: expected-result
    text: Expected result
    level: 2
  - id: example
    text: Example
    level: 2
  - id: caveats
    text: Caveats
    level: 2
---

A campaign searches the collections currently marked **subscribed** for it.

<h2 id="set-access">Set access</h2>

1. Open **Campaign & sources** and select the campaign.
2. Under **Source collections**, turn on the switch for every shelf it should use.
3. Expand a collection to inspect its **Books**, indexing state, and compiled **Rules**.
4. Turn the switch off when that campaign should stop using the collection.

<h2 id="expected-result">Expected result</h2>

Questions in that campaign can draw from relevant PDF passages, compiled rules, and collection material in the subscribed shelves. Unsubscribing changes access; it does not delete the collection or its books.

<h2 id="example">Example</h2>

Subscribe **The Brass Orchard** to **Orchard Almanac** and **Common Procedures**, but leave **Deep-Sea Bestiary** off. Ask:

> What opens the western seed vault?

Chronacle searches the two subscribed collections, not the unrelated bestiary.

<h2 id="caveats">Caveats</h2>

- Subscriptions are per campaign even though collections are shared.
- A book still being indexed may not contribute useful passages yet.
- Use [Organize source collections](/en/manual/source-library/collections) to add books to a shelf.
