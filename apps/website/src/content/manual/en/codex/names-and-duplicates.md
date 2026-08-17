---
translationKey: codex.identity
locale: en
slug: codex/names-and-duplicates
title: Resolve names and duplicates
summary: Use alternate names, review uncertain links, and merge records only when they truly describe the same thing.
section: codex
order: 6
headings:
  - id: add-an-alternate-name
    text: Add an alternate name
    level: 2
  - id: review-conflicts
    text: Review conflicts
    level: 2
  - id: example
    text: Example
    level: 2
  - id: merge-consequences
    text: Merge consequences
    level: 2
---

Alternate names make several names point to one entity; Maintenance lets you inspect ambiguous names and possible duplicates before changing anything.

<h2 id="add-an-alternate-name">Add an alternate name</h2>

Open an entity, add a value under **Alternate names**, and save. Links using the primary or alternate name can then resolve to that entity. An automatic unambiguous match appears under **Auto-linked** in Maintenance, where **Undo** removes the added alias.

<h2 id="review-conflicts">Review conflicts</h2>

1. Open **Maintenance → Findings** and choose **Check campaign** for a fresh check.
2. For a possible name mismatch, use **Use suggestion**, **Create article**, **Open source**, or **Dismiss**.
3. For a **Naming conflict**, choose which entity keeps a disputed alias when the available action is shown.
4. For a **Possible duplicate**, use **Open A** and **Open B** before **Merge**.

<h2 id="example">Example</h2>

Your notes link `[[Bellkeeper]]`, while the saved NPC is **Iria Pell, Keeper of Bells**. Add **Bellkeeper** as an alternate name, or confirm the suggested match in Maintenance. If a second **Iria Pell** record exists, compare both and merge only if they are the same person.

<h2 id="merge-consequences">Merge consequences</h2>

- You choose the surviving record and how to combine summary and notes.
- Relationships and names are carried to the survivor; duplicate edges are collapsed.
- The other record is removed from normal use, and the survivor's article is marked for recompilation.
