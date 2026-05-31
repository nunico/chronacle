# Source Collections Design

**Date:** 2026-05-31
**Status:** Approved
**Phase:** 1 / early Phase 2

## Problem

The current binary global/campaign model for PDF sources has two failure modes:

1. **Cross-system and cross-setting bleed.** A PDF uploaded as "global" appears in retrieval for every campaign — including unrelated game systems and settings.
2. **Duplicate indexing.** The only way to share a rulebook across multiple campaigns is to upload and re-index it for each one.

A three-tier (global / system / campaign) model was considered and rejected: it solves cross-system bleed but not cross-setting bleed within a system. For example, Forgotten Realms lore scoped to "D&D 5e" would still appear in a Dark Sun (also D&D 5e) campaign.

## Solution: Source Collections

Sources are organised into named **collections**. A campaign subscribes to any combination of collections. Retrieval searches only the collections the active campaign subscribes to.

This replaces the global/campaign scope binary entirely. "Universal" sources simply become a collection subscribed to by every campaign that wants them.

**Examples:**

| Collection | Subscribed by |
|---|---|
| D&D 5e Core Rules | Waterdeep Dragon Heist, Curse of Strahd, Dark Sun |
| D&D 5e Forgotten Realms | Waterdeep Dragon Heist |
| D&D 5e Dark Sun | Dark Sun |
| Pathfinder 2e Core | Heroes of Golarion |
| GM Advice | all campaigns |

---

## Data Model Changes

### New table: `collection`

```surql
DEFINE TABLE collection SCHEMAFULL;
DEFINE FIELD name        ON collection TYPE string;
DEFINE FIELD description ON collection TYPE string | NULL;
DEFINE FIELD created_at  ON collection TYPE datetime;
DEFINE FIELD updated_at  ON collection TYPE datetime;
```

### New RELATION table: `subscribes_to`

```surql
DEFINE TABLE subscribes_to TYPE RELATION SCHEMAFULL FROM campaign TO collection;
DEFINE FIELD created_at ON subscribes_to TYPE datetime;
```

### Modified table: `source`

Replace `campaign: record<campaign> | NULL` with a required collection link:

```surql
-- Remove: DEFINE FIELD campaign ON source TYPE record<campaign> | NULL;
DEFINE FIELD collection ON source TYPE record<collection>;
```

Every source must belong to exactly one collection. There are no unscoped (global) sources.

### Modified table: `chunk`

Inherit the collection from the source at index time:

```surql
-- Remove: DEFINE FIELD campaign ON chunk TYPE record<campaign> | NULL;
DEFINE FIELD collection ON chunk TYPE record<collection>;
```

### `campaign` table

No schema change. `campaign.system` remains a free-text string — a cosmetic label (shown in the campaign card, does not affect retrieval). It is not linked to any collection.

### Migration

No data migration required — the app has not been released. The schema migration file drops `source.campaign` / `chunk.campaign` and adds `source.collection` / `chunk.collection`.

---

## Retrieval Query

**Before:**

```surql
SELECT * FROM chunk
  WHERE campaign = $active_campaign OR campaign IS NULL
  ORDER BY embedding <|1|> $query_vector
  LIMIT 20
```

**After:**

```surql
LET $subs = (SELECT VALUE ->subscribes_to->collection FROM $active_campaign);
SELECT * FROM chunk
  WHERE collection IN $subs
  ORDER BY embedding <|1|> $query_vector
  LIMIT 20
```

The `LET` binds the campaign's subscribed collection IDs in one step. The service layer passes `$active_campaign` as a parameter; no separate resolution step is needed.

If the campaign subscribes to no collections, `$subs` is an empty array and the query returns nothing — correct behaviour (no bleed from other campaigns).

---

## IPC API Changes

### New commands — Collections

```
create_collection(name: String, description: Option<String>) -> Collection
get_collections() -> Vec<Collection>
update_collection(id: String, name: String, description: Option<String>) -> Collection
delete_collection(id: String) -> ()
  // Returns an error if any campaign is subscribed to this collection,
  // or if any source still belongs to it.
```

### New commands — Campaign subscriptions

```
add_campaign_collection(campaign_id: String, collection_id: String) -> ()
remove_campaign_collection(campaign_id: String, collection_id: String) -> ()
get_campaign_collections(campaign_id: String) -> Vec<Collection>
```

### Modified commands

```
// collection_id replaces campaign_id / game_system_id
// collection_id is now required (every source must belong to a collection)
upload_source(
  file_path: String,
  source_type: String,
  collection_id: String,
) -> Source

// null = all sources across all collections
get_sources(collection_id: Option<String>) -> Vec<Source>

// create_campaign / update_campaign: system remains a free-text String, no other change
// Collection subscriptions are managed separately via add/remove_campaign_collection
```

---

## Frontend / UX Changes

### Library sidebar (CampaignsPage)

New layout, top to bottom:

```
[ + New Collection ]

▾ D&D 5e Core Rules          ← collection (expandable; ⋯ menu: Rename, Delete)
    Player's Handbook.pdf
    Monster Manual.pdf
▾ D&D 5e Forgotten Realms
    Sword Coast Adventurer's Guide.pdf
▾ GM Advice
    Return of the Lazy GM.pdf

[ + New Campaign ]

▾ Waterdeep Dragon Heist  ·  D&D 5e
    Uses: D&D 5e Core Rules · D&D 5e Forgotten Realms · GM Advice
▾ Dark Sun: Last King       ·  D&D 5e
    Uses: D&D 5e Core Rules · GM Advice
```

Collection rows have an inline `⋯` menu with Rename and Delete. Delete is disabled (tooltip) if any campaign is subscribed or any source is in the collection.

The "Uses:" line in a campaign row lists subscribed collection names as chips; clicking the campaign opens it and its collection list is editable inline.

### Upload dialog

Single scope selector — collection picker:

```
Add to collection: [ D&D 5e Core Rules ▼ ] [ + Create new… ]
```

- No radio buttons needed; there is only one axis.
- Default: most recently used collection (MRU).
- "Create new…" opens an inline input for the collection name, creates it on confirm, and selects it.
- Created collections are not automatically subscribed to any campaign — the user subscribes campaigns separately in the campaign form.

### Campaign create / edit dialog

```
Name:             [ Waterdeep Dragon Heist        ]
Game system:      [ D&D 5e                        ]   ← free-text label, optional
Source collections:
  ✓ D&D 5e Core Rules
  ✓ D&D 5e Forgotten Realms
  ✓ GM Advice
  [ + Add collection ]  [ Create new collection… ]
```

Multi-select. "Create new collection…" behaves identically to the inline create in the upload dialog.

---

## User Guide

A new user guide page covers:

- **What collections are** — named groups of PDFs, independent of any single campaign. A collection exists on its own; campaigns subscribe to it.
- **Setting up collections** — create your first collection ("D&D 5e Core Rules"), upload your rulebooks to it, then create a campaign and subscribe it.
- **Sharing sources across campaigns** — subscribe multiple campaigns to the same collection. The PDFs are indexed once and searched for all of them.
- **Keeping settings separate** — create separate collections per setting (e.g., "D&D 5e Forgotten Realms" and "D&D 5e Dark Sun"). Subscribe each campaign only to the collections it needs.
- **Uploading campaign-specific material** — create a campaign-named collection (e.g., "Waterdeep Dragon Heist — Session Notes") and subscribe only that campaign to it.
- **What gets searched when you ask a question** — plain-language: "Chronacle only searches the collections your active campaign subscribes to. Nothing leaks from other campaigns or unsubscribed collections."
- **Renaming and deleting collections** — why Delete is blocked when sources or campaigns are still linked.

The user guide page is authored by the `user-guide-writer` agent as part of implementation.

---

## Out of Scope

- **`is_gm_only` on collections or sources:** deferred to Phase 2 along with all other `is_gm_only` work.
- **Reordering subscribed collections:** retrieval is unordered (vector search); collection order within a campaign has no effect.
- **Moving a source between collections:** not supported in Phase 1. Delete and re-upload to the target collection.
- **Per-query collection enable/disable toggle:** planned for Phase 3; the collection model makes this straightforward to add (filter `$subs` before retrieval).

---

## Testing

- **Unit:** `CollectionService` CRUD; `add_campaign_collection` / `remove_campaign_collection` happy and error paths; retrieval query parameter assembly (subscribed collection IDs resolved from campaign).
- **Integration:** create two collections; upload sources to each; create two campaigns subscribed to different combinations; verify each campaign's retrieval sees only its subscribed sources and nothing from the other collection.
- **Schema:** confirm `source.collection` is non-nullable (inserting a source without a collection_id returns a schema error).
- **Delete guard:** attempt to delete a collection with an active subscription or existing source — assert error returned; source and campaign unchanged.
- **Frontend component:** upload dialog MRU default; inline collection create flow; campaign form multi-select add/remove; Delete button disabled state.
