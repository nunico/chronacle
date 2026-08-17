---
translationKey: sources.upload
locale: en
slug: source-library/upload-pdfs
title: Upload PDFs
summary: Import one PDF into a collection and follow its progress until it is ready to search.
section: source-library
order: 3
headings:
  - id: import-a-pdf
    text: Import a PDF
    level: 2
  - id: expected-result
    text: Expected result
    level: 2
  - id: example
    text: Example
    level: 2
  - id: supported-content-and-errors
    text: Supported content and errors
    level: 2
---

<h2 id="import-a-pdf">Import a PDF</h2>

Choose one PDF, place it in a collection, and wait for the upload strip to report that it is ready.

1. Select **Upload PDF** in the side rail, **Attach a rulebook** beside the Oracle question field, or **Add book** inside an expanded collection.
2. Choose one `.pdf` file in the system picker.
3. If you started from the rail or Oracle, select a collection in **Add “filename” to collection**. You can also choose **Create new collection** there.
4. Select **Upload**. When you start from **Add book**, Chronacle uses that collection directly.
5. Watch the filename, status text, and **Upload progress** bar beneath the main view.
6. Open **Campaign & sources**, expand the collection, and confirm that the book shows **Indexed**.

<h2 id="expected-result">Expected result</h2>

The progress strip reaches **Ready!** and clears after a short pause; the source remains listed in its collection as **Indexed**.

<h2 id="example">Example</h2>

Select **Upload PDF**, choose `The Violet Ferry.pdf`, create the collection **River Mysteries**, and select **Upload**. After the book shows **Indexed**, subscribe **The Last Crossing** to the collection and ask:

> What must a passenger leave on the violet ferry?

Chronacle can answer with `[Source: "The Violet Ferry.pdf", p.12]`.

<h2 id="supported-content-and-errors">Supported content and errors</h2>

- Chronacle reads the text layer in a PDF. Image-only scans do not provide readable text; the import may end in **Error** or finish without useful searchable passages.
- Layouts such as columns, tables, and decorative pages can affect extracted reading order. Check the cited passage before relying on exact wording.
- Only one upload runs at a time. Starting another while one is active shows **An upload is already in progress — wait for it to finish.**
- An error remains visible until you dismiss it. Use its displayed message when deciding what to retry; do not assume the file is the cause.
