---
translationKey: vault.files
locale: en
slug: vault/file-format
title: Understand vault files
navTitle: File format
summary: Edit the parts that belong to you while leaving identity and generated content intact.
section: vault
order: 2
headings:
  - id: read-the-file
    text: Read the file
    level: 2
  - id: edit-safe-regions
    text: Edit safe regions
    level: 2
  - id: example
    text: Example
    level: 2
  - id: avoid-damage
    text: Avoid damage
    level: 2
---

Edit summaries, notes, table notes, and alternate names. Leave the `id`, generated metadata, and the fenced compiled text alone: Chronacle restores its canonical version after applying your supported edits.

<h2 id="read-the-file">Read the file</h2>

Every managed file starts with metadata between two `---` lines. The `id` is the stable link to the Chronacle record; the filename and folder path are not its identity. Entity files can then contain **Summary**, a compiled block, and **Notes**. Rule files have a compiled block and table notes. Session bodies are entirely your notes and have no compiled block.

Chronacle manages these exact fence lines:

```text
<!-- chronacle:codex-article start -- compiled; edits are not applied -->
<!-- chronacle:codex-article end -->
```

<h2 id="edit-safe-regions">Edit safe regions</h2>

1. Keep the opening metadata block and its `id` intact.
2. For an entity, edit the text under `## Summary` or `## Notes`.
3. For an entity or rule, add alternate names to the `aliases` list; keep the canonical name in that list.
4. Save the file, then wait for Chronacle or choose **Sync now**.

**Result:** supported fields are applied, then Chronacle rewrites the file in its standard form. Edits inside the compiled fence and changes to generated metadata are not applied.

<h2 id="example">Example</h2>

In Seraphina Aldric’s file, you change the summary to “Archivist of the Iron Tower and keeper of the dusk ledger,” add a table observation under `## Notes`, and leave the compiled block untouched. Chronacle imports the summary and notes, keeps its compiled article, and rewrites the metadata consistently.

<h2 id="avoid-damage">Avoid damage</h2>

- A managed file with missing or unreadable metadata is counted as invalid and is not applied or overwritten during that check.
- Removing a user-owned Summary or Notes section clears that field in Chronacle.
- Text in an unrecognised body section is treated as notes so it is not silently discarded.
- Do not copy one file’s `id` into another. Back up the folder before bulk edits.
