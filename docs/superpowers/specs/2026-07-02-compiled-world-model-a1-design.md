# Compiled World Model — PR-A1: Campaign-owned collections + two-mode delete

**Date:** 2026-07-02
**Status:** Approved (design). Implementation in progress.
**Area:** `src-tauri/src/schema/`, `src-tauri/src/services/campaign_service.rs`,
`src-tauri/src/services/collection_service/`, `src-tauri/src/commands/campaign_commands.rs`
**Roadmap:** Foundations for the LLM Wiki compiled knowledge layer (ADR-009,
ADR-010 — see the "b-compile" plan for the wiki itself).

## Problem

Chronacle currently treats **campaigns** and **collections** as two flat,
unrelated things:

- A campaign owns *nothing directly*. It is a name + system + a set of
  `subscribes_to` edges pointing at collections.
- Every collection is "regular" — a book, a bestiary, a lore compendium — and
  can be shared across many campaigns.

Real play does not work that way. Every campaign accumulates **campaign-only**
material — the party's actual NPCs, their actual locations, their actual
factions, their session log — that belongs *only* to that campaign. Today users
either dump this into a general-purpose collection (contaminating a shared
resource) or manually create a "MyCampaign notes" collection every time.

The compiled world model (wiki + rules; see follow-up specs) has to live
somewhere. It must live inside a collection that is:

1. **Auto-created** with the campaign (so the wiki has a home from day one).
2. **Owned** by the campaign (so retrieval knows which entries are the
   campaign's own truth vs. a shared reference).
3. **Not shareable** to other campaigns (so we don't leak spoilers).
4. **Deletable-with-escape**: when the campaign is deleted, the user may want
   to keep the collection as a regular one (e.g. to reuse the world for a new
   campaign).

This PR — **A1** — adds only the ownership plumbing and the two-mode delete.
The wiki and rules aggregates themselves land in A2.

## Goals

- One new field, `collection.owner_campaign`, that names the owning campaign
  (or is unset, meaning "regular / shared collection").
- Creating a campaign auto-creates its owned collection and subscribes the
  campaign to it. The owned collection is indistinguishable from any other in
  the UI *except* that it cannot be unsubscribed from its owner and cannot
  gain a second owner.
- Deleting a campaign asks: **cascade delete** the owned collection too, or
  **convert to regular** (drop `owner_campaign`, keep the collection and all
  its content)?
- Convert-to-regular **orphans campaign-bound↔campaign-bound edges** inside the
  owned collection, logging each dropped edge to `lint_finding` so the user
  can inspect what was lost.

## Non-goals

- No wiki, no rules, no compile step, no lint UI. (A2, B*, C*.)
- No is_gm_only. (Deferred.)
- No vault sync. (D2.)
- No changes to the *shape* of `relates_to` or `in_collection` edges.
- No changes to how existing regular collections behave.

## Domain model changes

### `collection` table — one new field

```
DEFINE FIELD OVERWRITE owner_campaign ON TABLE collection TYPE option<record<campaign>>;
DEFINE INDEX OVERWRITE collection_owner_campaign_idx ON TABLE collection COLUMNS owner_campaign;
```

Semantics:

| `owner_campaign` | meaning                                                    |
|------------------|------------------------------------------------------------|
| unset            | **regular collection** — shareable, unchanged behaviour     |
| set              | **campaign-bound** — auto-created, cannot be re-owned      |

The field is intentionally `option<record<campaign>>` (not two booleans). A
collection is bound *to a specific campaign* — the identity matters for
retrieval and for the delete cascade.

### `lint_finding` table (introduced early)

`lint_finding` is a full A2 concept, but A1 already produces one kind of finding
(orphaned edges after convert-to-regular). Introducing a minimal, additive
table stub now means no information is lost across the PR boundary. A2 will
extend `kind` and add more producers.

```
DEFINE TABLE lint_finding SCHEMAFULL;
DEFINE FIELD OVERWRITE kind ON TABLE lint_finding TYPE string;
DEFINE FIELD OVERWRITE payload ON TABLE lint_finding TYPE object;
DEFINE FIELD OVERWRITE created_at ON TABLE lint_finding TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE resolved_at ON TABLE lint_finding TYPE option<datetime>;
DEFINE INDEX OVERWRITE lint_finding_kind_idx ON TABLE lint_finding COLUMNS kind;
DEFINE INDEX OVERWRITE lint_finding_unresolved_idx ON TABLE lint_finding COLUMNS resolved_at;
```

Only one `kind` is populated in A1: `"orphaned_edge"`, with `payload`:

```
{
  "campaign_id":  "campaign:<id>",
  "collection_id":"collection:<id>",
  "edge_id":      "relates_to:<id>",
  "from":         "<entity table>:<id>",
  "to":           "<entity table>:<id>",
  "rel_type":     "<string>"
}
```

## Behaviour changes

### `campaign_service::create`

Existing: create campaign only.

New: within a single logical operation,

1. Create the `campaign` record (existing code path).
2. Create a `collection` with `name = campaign.name`, `owner_campaign = campaign.id`.
3. `RELATE campaign->subscribes_to->collection`.
4. Return the campaign as before.

No new return type. Callers that need the owned collection look it up via a
new `collection_service::owned_by(campaign_id)` helper (already trivial with
the new index).

If any of the three steps fails, the whole operation must appear atomic to
the caller. SurrealDB embedded does not give us multi-statement transactions
across our current `.query()` calls with strong guarantees, so we chain them
in a single `.query()` block and `UPDATE`/`DELETE` on failure — see the plan
doc for the exact query.

### `campaign_service::delete`

Signature becomes:

```rust
pub enum OnOwnedCollection {
    Delete,           // cascade: delete the owned collection and its content
    ConvertToRegular, // keep collection; drop owner_campaign; orphan intra edges
}

pub async fn delete(
    db: &Surreal<Db>,
    id: &str,
    on_owned_collection: OnOwnedCollection,
) -> Result<(), String>;
```

**Cascade path (`Delete`):**
1. Find owned collection (if any).
2. Delete all entities `WHERE in_collection = <owned>` and all their edges
   (`relates_to` in either direction, plus scope edges). This mirrors the
   existing delete-collection behaviour once — see `collection_service::delete`.
3. Delete the `subscribes_to` edge.
4. Delete the collection.
5. Delete the campaign.

**Convert path (`ConvertToRegular`):**
1. Find owned collection.
2. For every `relates_to` edge where **both** endpoints have
   `->in_collection->` pointing at the owned collection, `CREATE` a
   `lint_finding` (`kind = "orphaned_edge"`) and `DELETE` the edge.
   Edges with only one endpoint inside are **preserved** — they now cross into
   a regular collection, which is a legal state.
3. `UPDATE collection SET owner_campaign = NONE`.
4. Delete the `subscribes_to` edge (the campaign is going away).
5. Delete the campaign.

If the campaign has no owned collection (legacy data — none exists today, but
future imports may), both paths degrade to "just delete the campaign."

### Tauri commands

- `create_campaign` — unchanged signature; behaviour now includes owned collection.
- `delete_campaign(id, on_owned_collection: "delete" | "convert_to_regular")` —
  gains second parameter. Default when omitted: **error** (force the frontend to
  make the choice explicit). This is deliberate — silently defaulting to
  cascade would destroy user data.

### Frontend

`CampaignView.svelte::removeCampaign` replaces the current `confirm()` with a
two-button dialog:

- "Delete campaign and its notes"
- "Delete campaign, keep notes as a regular collection"
- "Cancel"

The choice maps 1:1 to the new command parameter.

## Retrieval implications (none in A1)

Nothing yet reads `owner_campaign`. Retrieval changes land in the wiki
integration PR (B3). A1 is pure plumbing.

## Migration

Squashed schema stays single-file per repo convention. A1 adds
`002_wiki_layer.surql` **but only its A1 slice** — the file exists, but only
contains: the `collection.owner_campaign` field + index, and the
`lint_finding` table + fields + indexes. A2 will extend the same file (all
`DEFINE ... OVERWRITE`, per repo convention, so re-runs are idempotent).

Existing databases: every existing collection has `owner_campaign = NONE`
after the migration. Nothing needs backfill.

## Test plan (BDD-flavoured, TDD-ordered)

Failing tests are written before implementation. Named tests match the
"Given/When/Then" ordering used in the existing test suite.

**Schema-level (integration, in-memory SurrealDB):**
- `migration_is_idempotent_when_run_twice`
- `collection_owner_campaign_is_optional_and_defaults_none`
- `lint_finding_table_exists_and_accepts_orphaned_edge_row`

**`campaign_service::create`:**
- `creating_campaign_auto_creates_owned_collection_with_matching_name`
- `creating_campaign_auto_subscribes_to_owned_collection`
- `owned_collection_lookup_returns_it`

**`campaign_service::delete` — cascade:**
- `delete_cascade_removes_owned_collection`
- `delete_cascade_removes_entities_inside_owned_collection`
- `delete_cascade_leaves_regular_collections_untouched`

**`campaign_service::delete` — convert:**
- `delete_convert_keeps_owned_collection_but_drops_owner_field`
- `delete_convert_orphans_only_intra_owned_edges`
- `delete_convert_logs_orphaned_edge_findings`
- `delete_convert_preserves_edges_crossing_into_regular_collections`

**Command layer (thin):**
- `delete_campaign_command_requires_on_owned_collection_choice`

## Risks & tradeoffs

- **Widening A1 with `lint_finding`.** Minor; it is one additive table with no
  cross-references. Worth it to not lose orphan information.
- **Silently defaulting delete to cascade.** Rejected — see command section.
  Requiring the choice up front is a one-time frontend change and prevents
  data loss.
- **Convert-to-regular leaves half-edges.** By design — an edge from a
  campaign NPC into a shared bestiary creature is legitimate cross-scope
  information and must survive. The user chose "convert", meaning "treat
  this like a normal collection now."
- **Race between step 4 and step 5 in convert path.** Single-node embedded
  SurrealDB; not a real concern.

## Follow-ups (explicitly out of scope)

- Wiki + rules aggregates (A2).
- Compile step (B1, B2).
- Retrieval integration (B3).
- Lint UI + more producers (C1, C2, D1).
- Vault sync (D2).
