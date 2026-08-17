---
translationKey: vault.deleting
locale: en
slug: vault/deleting
title: Delete records and vault files
navTitle: Deleting
summary: Understand which side a deletion affects and what Chronacle deliberately leaves behind.
section: vault
order: 5
headings:
  - id: delete-in-chronacle
    text: Delete in Chronacle
    level: 2
  - id: delete-in-the-folder
    text: Delete in the folder
    level: 2
  - id: example
    text: Example
    level: 2
  - id: protect-your-work
    text: Protect your work
    level: 2
---

Treat deleting as consequential: there is currently no restore screen. A user-facing delete hides the record throughout Chronacle; a full folder check removes its unchanged managed file but preserves a file that you edited after the last shared version.

<h2 id="delete-in-chronacle">Delete in Chronacle</h2>

1. Back up any notes you may need.
2. Use the record’s remove action and confirm the warning.
3. If the managed file does not disappear promptly, choose **Settings → Markdown vault → Sync now**.

**Result:** the record is hidden from lists, search, links, and Codex compilation. Its unchanged managed file and any conflict sidecar are cleaned up. A managed file whose contents no longer match the last shared version is left on disk so your later prose is not overwritten or deleted.

<h2 id="delete-in-the-folder">Delete in the folder</h2>

Deleting a previously exported managed file signals that you want its Chronacle record hidden. Before acting, Chronacle scans for the same `id` elsewhere; this avoids mistaking an editor’s temporary remove-and-recreate save for a deletion and allows a genuine move to be found.

If no file with that `id` remains, the next full check soft-deletes the record. “Soft-delete” means Chronacle keeps the underlying record but excludes it everywhere you can currently use it. No undelete action is implemented.

<h2 id="example">Example</h2>

You remove the abandoned NPC Orren Pike in Chronacle. On reconciliation, `campaigns/shadows-of-valdris/entities/npc/orren-pike.md` is deleted if it still matches Chronacle’s last export. If you had added an unsaved epilogue in that file, the file remains, even though Orren is hidden in Chronacle.

<h2 id="protect-your-work">Protect your work</h2>

- Make a separate backup before deleting many records or files.
- Do not delete a `.conflict.md` file as cleanup; that is a conflict-resolution signal.
- A renamed file with its original `id` is a move, not a deletion.
- **Disconnect** is the safe choice when you only want to stop using the folder; it leaves files in place.
