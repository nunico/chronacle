---
translationKey: vault.conflicts
locale: en
slug: vault/conflicts
title: Resolve vault conflicts
navTitle: Conflicts
summary: Compare both preserved versions, merge deliberately, and unfreeze the record safely.
section: vault
order: 4
headings:
  - id: recognise-a-conflict
    text: Recognise a conflict
    level: 2
  - id: resolve-it
    text: Resolve it
    level: 2
  - id: example
    text: Example
    level: 2
  - id: if-resolution-fails
    text: If resolution fails
    level: 2
---

A conflict means the Chronacle record and its Markdown file both changed since their last shared version. Chronacle freezes that record, leaves your file untouched, and writes its own current version beside it as `<name>.conflict.md`.

<h2 id="recognise-a-conflict">Recognise a conflict</h2>

Open **Settings → Markdown vault**. The **Conflicts** list shows the record name, kind, normal path, and `.conflict.md` path. An affected entity editor also shows a conflict banner. Repeated checks do not choose a winner while the sidecar exists.

<h2 id="resolve-it">Resolve it</h2>

1. Back up both files before editing or deleting either one.
2. Compare the normal `.md` file—your folder version—with the adjacent `.conflict.md` file—Chronacle’s version.
3. Put the final text you want to keep into the supported user-owned regions of the normal file.
4. Save the normal file, then delete only the matching `.conflict.md` file.
5. Choose **Sync now**.

**Result:** Chronacle takes the normal file’s supported fields, clears the freeze, and returns the record to normal folder checks.

<h2 id="example">Example</h2>

You add “Mara met the ferryman” to `mara-venn.md` while also changing Mara’s notes inside Chronacle. The next check creates `mara-venn.conflict.md`. Compare both, combine the two facts under `## Notes` in `mara-venn.md`, save, delete `mara-venn.conflict.md`, and run **Sync now**.

<h2 id="if-resolution-fails">If resolution fails</h2>

- If the normal file cannot be read after sidecar deletion, Chronacle restores the sidecar and keeps the record frozen. Repair the metadata or body, then repeat the safe steps.
- If you make the normal file match Chronacle’s version exactly, the conflict can clear and the sidecar can be removed automatically.
- Do not delete the sidecar until you have copied any Chronacle-side text you need. Its deletion is the explicit signal to prefer the normal file.
- A conflict identifies two changed versions; it does not by itself reveal which edit or program caused them.
