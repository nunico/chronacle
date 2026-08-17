---
translationKey: sources.ingestion
locale: en
slug: source-library/indexing
title: Understand indexing
summary: Follow the visible indexing states and know when a source is ready, failed, or needs rebuilding.
section: source-library
order: 4
headings:
  - id: what-indexing-does
    text: What indexing does
    level: 2
  - id: expected-result
    text: Expected result
    level: 2
  - id: example
    text: Example
    level: 2
  - id: states-and-reindexing
    text: States and re-indexing
    level: 2
---

<h2 id="what-indexing-does">What indexing does</h2>

Indexing reads the PDF's text page by page, divides it into searchable passages, and stores a local search index with source and page details.

1. Start an import and watch the upload strip for the current filename, step, and percentage.
2. During work, the strip can show messages such as **Uploading…**, **Indexing PDF…**, or a more specific processing step.
3. Wait for **Ready!** before asking the Oracle to use the new source.
4. In **Campaign & sources**, expand the collection and check the book state: **Indexing…**, **Indexed**, or **Error**.
5. After changing **Embedding mode**, open **Settings** and choose **Re-index all sources**. Follow the source count, step, and percentage shown there.

<h2 id="expected-result">Expected result</h2>

A successful source ends as **Indexed** and becomes searchable by campaigns subscribed to its collection; a failure shows **Error** and leaves a visible message to inspect.

<h2 id="example">Example</h2>

Import `The Clockmaker's Wake.pdf` into **City Mysteries**. Wait until it changes from **Indexing…** to **Indexed**, subscribe **The Thirteenth Chime**, and ask:

> Which gear opens Master Pell's hidden workshop?

Chronacle can retrieve the indexed passage and cite `[Source: "The Clockmaker's Wake.pdf", p.48]`.

<h2 id="states-and-reindexing">States and re-indexing</h2>

- **Indexed** means the current import completed. It does not guarantee that every complex layout was read in the intended order.
- **Error** means indexing did not complete successfully. Read the displayed error before retrying.
- Image-only PDFs have no text layer for the current extractor; optical character recognition is not part of the import flow.
- If Chronacle reports that sources use another indexing model, use **Re-index now** in the warning or **Re-index all sources** in Settings. Existing sources remain searchable during that re-index operation, and their stored passages are replaced as each source completes.
