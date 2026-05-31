# Three-Tier Source Scope Design

**Date:** 2026-05-31
**Status:** Approved
**Phase:** 1 (can ship alongside or immediately after current Phase 1 work)

## Problem

The current binary global/campaign model for PDF sources has two failure modes:

1. **Cross-system bleed.** A D&D 5e rulebook uploaded as "global" appears in retrieval for every campaign, including Pathfinder ones.
2. **Duplicate indexing.** The only way to share a rulebook across multiple campaigns of the same system is to upload and re-index it for each campaign individually.

## Solution: Three Tiers

Every source (and its chunks) belongs to exactly one of three tiers:

| Tier | `source.game_system` | `source.campaign` | Searched when... |
|---|---|---|---|
| Universal | NULL | NULL | Always, for every campaign |
| System-scoped | `record<game_system>` | NULL | Active campaign's `game_system` matches |
| Campaign-scoped | NULL | `record<campaign>` | Active campaign matches |

A new `game_system` table is introduced as a first-class entity. Campaigns link to a game system via a record link (replacing the old free-text `system` string). Linking is optional — a campaign with no game system sees only its own sources and universal ones.

---

## Data Model Changes

### New table: `game_system`

```surql
DEFINE TABLE game_system SCHEMAFULL;
DEFINE FIELD name ON game_system TYPE string;
DEFINE FIELD created_at ON game_system TYPE datetime;
DEFINE FIELD updated_at ON game_system TYPE datetime;
```

### Modified table: `source`

Add `game_system` field (mutually exclusive with `campaign` — enforced in the service layer):

```surql
DEFINE FIELD game_system ON source TYPE record<game_system> | NULL;
```

### Modified table: `chunk`

Add `game_system` field, inherited from the source at index time:

```surql
DEFINE FIELD game_system ON chunk TYPE record<game_system> | NULL;
```

### Modified table: `campaign`

Replace `system: string` with `game_system: record<game_system> | NULL`:

```surql
-- Remove:
-- DEFINE FIELD system ON campaign TYPE string;

-- Add:
DEFINE FIELD game_system ON campaign TYPE record<game_system> | NULL;
```

### Data migration

For any existing campaigns that carry a `system` string value:

1. Collect all distinct `system` strings across all campaign records.
2. For each distinct string, create one `game_system` record with that name.
3. For each campaign, set `game_system` to the matching record ID.
4. Remove the old `system` field from the campaign schema.

This migration runs in the startup schema-apply step. If the `system` field is already absent (fresh install), the migration is a no-op.

---

## Retrieval Query Change

**Before:**

```surql
SELECT * FROM chunk
  WHERE campaign = $active_campaign OR campaign IS NULL
  ORDER BY embedding <|1|> $query_vector
  LIMIT 20
```

**After:**

```surql
SELECT * FROM chunk
  WHERE campaign = $active_campaign
     OR (game_system = $active_game_system AND campaign IS NULL)
     OR (game_system IS NULL AND campaign IS NULL)
  ORDER BY embedding <|1|> $query_vector
  LIMIT 20
```

`$active_game_system` is the `game_system` record ID from the active campaign, resolved by the service layer before the query runs (one point-lookup on the campaign record). If the campaign has no linked game system, `$active_game_system` is `NONE` and the middle clause matches nothing — correct behaviour.

This also fixes a pre-existing correctness bug: the old `campaign IS NULL` catch-all included all global sources in every query regardless of system.

---

## IPC API Changes

### New commands

```
create_game_system(name: String) -> GameSystem
get_game_systems() -> Vec<GameSystem>
update_game_system(id: String, name: String) -> GameSystem
delete_game_system(id: String) -> ()
  // Returns an error if any source or campaign is still linked to this system.
```

### Modified commands

```
// Before: create_campaign(name, system: String)
// After:
create_campaign(name: String, game_system_id: Option<String>) -> Campaign
update_campaign(id: String, name: String, game_system_id: Option<String>) -> Campaign

// Scope is now three-way: provide exactly one of game_system_id / campaign_id / neither.
// Both null = universal.
upload_source(
  file_path: String,
  source_type: String,
  game_system_id: Option<String>,
  campaign_id: Option<String>,
) -> Source

// Scope filter: both null = universal only; provide one to filter by system or campaign.
// Pass game_system_id = "*" or campaign_id = "*" to get all of that tier.
get_sources(game_system_id: Option<String>, campaign_id: Option<String>) -> Vec<Source>
```

On the frontend, `SourceScope` is represented as:

```ts
type SourceScope =
  | { kind: "universal" }
  | { kind: "system"; id: string }
  | { kind: "campaign"; id: string }
  | { kind: "all" }
```

The backend infers scope from which of `game_system_id` / `campaign_id` is set.

---

## Frontend / UX Changes

### Library sidebar (CampaignsPage)

New layout, top to bottom:

```
[ + Add Game System ]

▾ D&D 5e                         ← game_system section (expandable)
    Player's Handbook.pdf
    Monster Manual.pdf
▾ Pathfinder 2e
    Core Rulebook.pdf

[ + Add Campaign ]

▾ Waterdeep Dragon Heist         ← campaign (shows game system badge)
    Waterdeep Lore.pdf
▾ Curse of Strahd
    (no campaign sources yet)

── Universal ──                   ← collapsed by default, shown at bottom
    GM Advice Book.pdf
```

Game system rows have an inline `⋯` menu with Rename and Delete. Delete is disabled (with a tooltip) if any sources or campaigns are still linked.

### Upload dialog

Scope selector becomes a three-way radio:

```
( ) System-scoped   ( ) Campaign-scoped   ( ) Universal
```

- **Default:** Campaign-scoped, pre-selected to the active campaign.
- If no active campaign exists: defaults to System-scoped with the first available game system selected.
- System-scoped → shows game system dropdown.
- Campaign-scoped → shows campaign dropdown.
- Universal → no additional selection.

### Campaign create / edit dialog

The free-text system field becomes a "Game System" dropdown populated from `get_game_systems()`. It includes a "Create new…" inline option that shows a name input without navigating away. The field is optional.

---

## User Guide

A new user guide page covers:

- **Setting up your game system library** — how to create a game system and upload core rulebooks to it; explains that those PDFs are automatically available in every campaign using that system.
- **Creating a campaign and linking it to a system** — walk through the game system picker; explains what happens when the field is left blank.
- **Uploading campaign-specific sources** — setting sourcebooks, handouts, adventure supplements that apply only to one campaign.
- **Universal sources** — when to use them (rare: truly system-agnostic reference material), and why they appear collapsed at the bottom by default.
- **What gets searched when you ask a question** — plain-language explanation of the three tiers: "When you're in your Waterdeep Dragon Heist campaign, Chronacle searches your campaign sources, all D&D 5e sources, and any universal sources — nothing from your Pathfinder campaigns."
- **Renaming and deleting game systems** — how to do it safely; why Delete is blocked when sources are still attached.

The user guide page is authored by the `user-guide-writer` agent as part of implementation.

---

## Out of Scope

- **Source collections / subscriptions (many-to-many):** e.g., a shared "Forgotten Realms" lore pack subscribed to by multiple campaigns. Natural Phase 2 extension; the three-tier model does not block it.
- **Per-query source enable/disable toggle:** planned for Phase 3; not affected by this change.
- **`is_gm_only` on game_system sources:** deferred to Phase 2 along with all other `is_gm_only` work.

---

## Testing

- **Unit:** `GameSystemService` CRUD; service-layer enforcement that `game_system` and `campaign` are mutually exclusive on a source; retrieval query parameter assembly (resolves `$active_game_system` from campaign lookup correctly, including the NONE case).
- **Integration:** upload system-scoped source → query from matching campaign → assert retrieved; query from non-matching campaign → assert not retrieved; universal source retrieved from both campaigns.
- **Migration test:** seed a campaign with a legacy `system` string; run migration; assert `game_system` record exists with that name and campaign links to it.
- **Frontend component:** upload dialog defaults correctly (campaign active vs. no campaign); game system dropdown in campaign form populates and submits; Delete button disabled state when sources are linked.
