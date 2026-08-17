---
translationKey: notes.notes
locale: en
slug: notes-and-sessions/notes
title: Take campaign notes
summary: Store people, places, events, and other campaign facts as editable linked entities.
section: notes-and-sessions
order: 1
headings:
  - id: create-a-note
    text: Create a note
    level: 2
  - id: what-persists
    text: What persists
    level: 2
  - id: example
    text: Example
    level: 2
  - id: current-limits
    text: Current limits
    level: 2
---

Campaign notes are saved as entities in eight notebooks: Player Characters, NPCs, Locations, Factions, Creatures, Items, Events, and Misc.

<h2 id="create-a-note">Create a note</h2>

1. Select a campaign, then choose a notebook in the left rail.
2. Choose **New NPC** (or the matching label for that notebook) and enter the required **Name**.
3. Add a **Summary**, **Notes**, and any fields shown for that kind.
4. Use `[[Entity Name]]` to link another saved entity, then choose **Save**.

<h2 id="what-persists">What persists</h2>

Your name, alternate names, summary, notes, and kind-specific fields are stored with the campaign. Editing them marks any generated **Codex Article** stale, but does not replace your writing. Saved relationships appear in the entity's **Relationships** section and graph.

<h2 id="example">Example</h2>

Create the NPC **Mara Venn** with summary “Ferrymaster of Greyharbor.” In **Notes**, write:

> Owes [[Iria Pell]] a favor after the North Quay fire. Promised the crew passage at moonrise.

After saving, the link can open Iria's record, and a campaign question can use Mara's stored facts as context.

<h2 id="current-limits">Current limits</h2>

- A note belongs to the selected campaign; switch campaigns to see another game's entities.
- The current entity list is separated by notebook kind rather than one combined list.
- Deleting an entity asks for confirmation and removes it from normal Chronacle views.
