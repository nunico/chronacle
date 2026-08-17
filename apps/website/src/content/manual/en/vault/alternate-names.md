---
translationKey: vault.aliases
locale: en
slug: vault/alternate-names
title: Add alternate names in the vault
navTitle: Alternate names
summary: Keep nicknames and titles searchable without changing a record's identity.
section: vault
order: 3
headings:
  - id: edit-aliases
    text: Edit aliases
    level: 2
  - id: how-names-round-trip
    text: How names round-trip
    level: 2
  - id: example
    text: Example
    level: 2
  - id: tips
    text: Tips
    level: 2
---

Add nicknames, titles, and former names to the frontmatter `aliases` list. Chronacle imports those alternate names and keeps the record’s own name first so `[[Name]]` links work in Markdown editors that understand aliases.

<h2 id="edit-aliases">Edit aliases</h2>

1. Open the managed entity or rule file.
2. Find `aliases` in the metadata block.
3. Keep the canonical name and add your alternatives, for example `aliases: ["Mara Venn", "The Lantern", "Captain Venn"]`.
4. Save and choose **Sync now**, or let the folder watcher notice the edit.

**Result:** Chronacle stores “The Lantern” and “Captain Venn” as alternate names, then rewrites a deduplicated list with “Mara Venn” first.

<h2 id="how-names-round-trip">How names round-trip</h2>

The canonical name in `aliases` supports links but is not stored as an alternate name. Chronacle removes duplicates with ASCII case-insensitive matching. Case variants containing characters such as `Ä` and `ä` may remain distinct. Session files also carry their title in `aliases`, but changing session aliases does not change a Chronacle field.

Renaming a file does not rename its record. The `id` identifies the record, and Chronacle can keep a user-renamed path when it remains inside a supported managed folder.

<h2 id="example">Example</h2>

The party knows Seraphina Aldric as “Keeper of the Dusk Ledger.” Add that phrase to `aliases`. A note containing `[[Keeper of the Dusk Ledger]]` can then resolve to Seraphina while her main name remains unchanged.

<h2 id="tips">Tips</h2>

- Put each alternate name in quotes and separate entries with commas.
- Do not edit `id` to merge or retarget records; use Chronacle’s entity tools.
- If two records claim the same alternate name, use the finding shown by Chronacle to decide which record should keep it.
- See [Names and duplicates](/en/manual/codex/names-and-duplicates) for in-app identity work.
