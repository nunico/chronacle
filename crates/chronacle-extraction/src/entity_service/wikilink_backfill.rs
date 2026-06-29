//! One-shot wikilink resync: re-resolve `[[wikilinks]]` across every existing
//! entity so forward references that never resolved at creation time become
//! edges now that all entities exist.

use serde::Deserialize;
use surrealdb::sql::Thing;

use super::{EntityError, SELECT_SCOPE_ALIASES};

/// Entity row returned when scanning for wikilink backfill. Scope aliases give
/// the campaign and collection the entity belongs to.
#[derive(Deserialize)]
struct WikilinkScanRow {
    id: Thing,
    notes: Option<String>,
    campaign: Option<Thing>,
    collection: Option<Thing>,
}

/// Re-run wikilink resolution over EVERY existing entity, resolving each in its
/// own scope (campaign or collection). Forward references that never resolved at
/// creation time become edges now that all entities exist. Returns the number of
/// entities processed. Per-entity failures are logged and skipped (never abort).
pub async fn resync_all_wikilinks<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
) -> Result<usize, EntityError> {
    // All 8 entity tables — must stay in sync with wikilink::ENTITY_TABLES.
    const TABLES: &[&str] = &[
        "npc",
        "location",
        "faction",
        "creature",
        "item",
        "event",
        "player_character",
        "misc",
    ];

    let mut processed = 0usize;

    for table in TABLES {
        let query = format!("SELECT id, notes, {SELECT_SCOPE_ALIASES} FROM {table}");
        let mut resp = db.query(query).await.map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
        let rows: Vec<WikilinkScanRow> = resp.take(0).map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;

        for row in rows {
            // Only entities with non-empty notes can have wikilinks.
            let notes = match row.notes {
                Some(ref n) if !n.is_empty() => n.clone(),
                _ => continue,
            };

            // Build owned scope-id strings first so we can borrow them when
            // constructing the WikilinkScope (which carries `&str`).
            let entity_id = row.id.id.to_raw();
            let scope_collection: Option<String> = row.collection.map(|t| t.id.to_raw());
            let scope_campaign: Option<String> = row.campaign.map(|t| t.id.to_raw());

            let result = if let Some(ref col_id) = scope_collection {
                crate::wikilink::parse_and_sync_wikilinks(
                    db,
                    table,
                    &entity_id,
                    &notes,
                    crate::wikilink::WikilinkScope::Collection {
                        collection_id: col_id,
                    },
                )
                .await
            } else if let Some(ref camp_id) = scope_campaign {
                crate::wikilink::parse_and_sync_wikilinks(
                    db,
                    table,
                    &entity_id,
                    &notes,
                    crate::wikilink::WikilinkScope::Campaign {
                        campaign_id: camp_id,
                    },
                )
                .await
            } else {
                // Global entity with no scope — cannot resolve wikilinks.
                continue;
            };

            match result {
                Ok(_) => processed += 1,
                Err(e) => eprintln!("resync_wikilinks: failed to sync {table}:{entity_id}: {e}"),
            }
        }
    }

    Ok(processed)
}
