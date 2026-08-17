---
translationKey: codex.compile
locale: en
slug: codex/compile
title: Compile a collection
summary: Generate or refresh entity articles and rule entries from one collection's indexed material.
section: codex
order: 2
headings:
  - id: start-a-compile
    text: Start a compile
    level: 2
  - id: expected-result
    text: Expected result
    level: 2
  - id: example
    text: Example
    level: 2
  - id: interruptions-and-limits
    text: Interruptions and limits
    level: 2
---

Choose **Compile** on a subscribed collection to generate stale or missing entity articles and distill rules from its indexed rules material.

<h2 id="start-a-compile">Start a compile</h2>

1. Open **Campaign & sources** and expand **Source collections**.
2. Subscribe to the collection if necessary.
3. Check that its books show **Indexed**.
4. Choose **Compile**. The button changes to **Cancel**, and a progress message appears.

<h2 id="expected-result">Expected result</h2>

Chronacle updates generated entity articles that had no article or were marked stale, then creates or updates compiled rule entries. Existing rules with the same name are updated rather than duplicated. Your entity notes and rule **Table notes** are preserved.

<h2 id="example">Example</h2>

After adding `Lantern District Addendum.pdf` to **Greyharbor Gazetteer**, compile the collection. The stale **Mara Venn** article can be refreshed with a citation to the addendum, and a new **Moon-Tide Crossing** procedure can appear under **Rules**.

<h2 id="interruptions-and-limits">Interruptions and limits</h2>

- **Cancel** stops the current run; work already saved before cancellation can remain.
- An entity with no relevant source passage is left unchanged and still needs attention.
- Rule extraction is generated work. Review the body and page references rather than treating it as exact source text.
