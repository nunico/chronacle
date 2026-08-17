---
translationKey: vault.switching
locale: en
slug: vault/switch-folder
title: Switch the vault folder
navTitle: Switch folder
summary: Move Chronacle to a different local folder without making the new folder look like a mass deletion.
section: vault
order: 6
headings:
  - id: prepare-the-switch
    text: Prepare the switch
    level: 2
  - id: choose-the-new-folder
    text: Choose the new folder
    level: 2
  - id: example
    text: Example
    level: 2
  - id: recover-from-failure
    text: Recover from failure
    level: 2
---

Back up both folders, then choose the new folder in **Settings → Markdown vault**. Chronacle treats a genuinely different path as a fresh baseline and immediately performs a full check there.

<h2 id="prepare-the-switch">Prepare the switch</h2>

1. Finish or copy aside any unresolved `.conflict.md` work in the current folder.
2. Close other tools that are making bulk changes to either folder.
3. Back up the current and destination folders.
4. Decide whether the destination is empty or already contains Chronacle files. Existing files with matching `id` values are compared, not blindly overwritten.

<h2 id="choose-the-new-folder">Choose the new folder</h2>

1. Select **Choose folder…** and pick the destination.
2. Wait for the immediate check to complete.
3. Inspect the folder and the conflict list before editing further.

**Result:** an empty destination receives a fresh export. Chronacle clears the old folder’s comparison baseline before checking the new folder so missing destination files do not become deletion signals. The old folder is no longer watched; switching does not delete its files.

Choosing the same path again keeps the existing baseline and checks ordinary changes instead of treating it as a fresh destination.

<h2 id="example">Example</h2>

You switch from `Valdris Notes` to an empty `Campaign Archive`. Chronacle exports Mara Venn, the Iron Tower, and session 012 into the new folder. The files in `Valdris Notes` remain, but later Chronacle edits go only to `Campaign Archive`.

<h2 id="recover-from-failure">Recover from failure</h2>

- If Chronacle cannot check the destination, the new path is not saved. The previous path and its comparison baseline remain in force, and the old folder watcher is restored.
- If the error says the previous vault’s sync baseline could not be restored, return to the old folder, run **Sync now**, and resolve any conflicts that appear. Preserve backups until the folder is settled.
- **Disconnect** clears the active path and stops folder activity without deleting either folder.
- A successful switch can create conflicts when destination files with matching identities contain different edits; resolve them normally rather than replacing files wholesale.
