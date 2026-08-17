---
translationKey: vault.overview
locale: en
slug: vault/overview
title: Keep a Markdown vault
navTitle: Vault overview
summary: Keep Chronacle records as local Markdown files that you can also edit in a text editor.
section: vault
order: 1
headings:
  - id: connect-a-folder
    text: Connect a folder
    level: 2
  - id: what-chronacle-manages
    text: What Chronacle manages
    level: 2
  - id: example
    text: Example
    level: 2
  - id: before-you-begin
    text: Before you begin
    level: 2
---

A Markdown vault mirrors campaign entities and sessions, plus collection entities and compiled collection rules, into a folder on this computer. Changes to the supported, user-owned parts can flow back into Chronacle.

<h2 id="connect-a-folder">Connect a folder</h2>

1. Make a backup of any existing folder you plan to use.
2. Open **Settings → Markdown vault** and choose **Choose folder…**.
3. Select a local folder. Chronacle immediately checks it and writes its records under `campaigns/` and `collections/`.
4. After later edits, choose **Sync now** when you want a full check. Chronacle also checks at startup and watches the connected folder while the app is running.

**Result:** the panel reports how many records were exported, unchanged, applied, conflicted, resolved, soft-deleted, invalid, or failed.

<h2 id="what-chronacle-manages">What Chronacle manages</h2>

Only four folder shapes are managed: campaign entities, campaign sessions, collection entities, and collection rules. Files elsewhere in the chosen folder—including the vault root and `.obsidian/`—are ignored.

Your summaries, notes, table notes, and alternate names are user-owned. Chronacle owns file identity and other generated metadata, plus text inside the marked compiled block. See [Understand the file format](/en/manual/vault/file-format) before editing a file and [Handle conflicts](/en/manual/vault/conflicts) before deleting a `.conflict.md` file.

<h2 id="example">Example</h2>

After connecting `Valdris Notes`, Chronacle writes Mara Venn to `campaigns/shadows-of-valdris/entities/npc/mara-venn.md`. You add “Promised the party safe passage through North Quay” under **Notes** in a text editor. On the next check, that note appears on Mara’s Chronacle record and becomes searchable.

<h2 id="before-you-begin">Before you begin</h2>

- This feature works with the local folder you select in Settings.
- **Disconnect** stops Chronacle from watching and writing the folder; it does not remove its files.
- Moving, deleting, or editing managed files can change or hide Chronacle records. Keep a separate backup before bulk file work.
- Folder changes have special recovery rules; read [Switch the vault folder](/en/manual/vault/switch-folder) first.
