---
translationKey: getting-started.quick-start
locale: en
slug: getting-started/quick-start
title: Quick start
summary: Install Chronacle, connect an answer provider, import a PDF, and check your first cited answer.
section: getting-started
order: 1
headings:
  - id: start-here
    text: Start here
    level: 2
  - id: expected-result
    text: Expected result
    level: 2
  - id: example
    text: Example
    level: 2
  - id: before-you-continue
    text: Before you continue
    level: 2
---

<h2 id="start-here">Start here</h2>

You can go from a new installation to a cited answer by completing the setup below.

1. [Install the current release](/en/manual/getting-started/install) and open Chronacle.
2. If the **AI model required** screen appears, choose **Start download** and wait for **Model ready!**
3. Open **Settings**, then [choose and connect an answer provider](/en/manual/ai-providers/choose).
4. [Import a text-based PDF](/en/manual/source-library/upload-pdfs) into a collection.
5. Create or select a campaign under **Campaign & sources**, then subscribe it to that collection.
6. Wait until the book shows **Indexed**, return to **Oracle**, and type a question.
7. Open a source badge in the answer to inspect the supporting passage.

<h2 id="expected-result">Expected result</h2>

The Oracle shows a concise answer with a clickable source badge naming the PDF and page; selecting the badge opens the source passage when one is available.

<h2 id="example">Example</h2>

Import `The Ashen Archive.pdf` into a collection named **Ember Coast**, subscribe the campaign **Lanterns at Dusk**, and type:

> What payment does Archivist Selka demand before opening the sealed stacks?

A grounded answer could say that Selka asks for a recovered brass index key, followed by a badge such as:

> [Source: "The Ashen Archive.pdf", p.42]

Select **The Ashen Archive.pdf p.42** to compare the answer with the passage.

<h2 id="before-you-continue">Before you continue</h2>

- The answer provider and the search index are separate settings. The first writes the reply; the second helps Chronacle find relevant passages.
- A campaign searches only the collections it is subscribed to.
- Image-only scans have no text layer for Chronacle to read. See [how indexing works](/en/manual/source-library/indexing) before troubleshooting a weak or empty result.
