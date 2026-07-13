use surrealdb::Connection;

use super::super::AgentError;
use super::format::format_entity_output;
use super::rows::{BasicRow, EventRow, PcRow, SessionRow};

/// Query entity tables for a campaign (and optionally subscribed collections)
/// and format them as a context block.
///
/// Campaign-scoped entities are always included in full. Collection-scoped
/// entities are retrieved via MTREE KNN search when `query_embedding` is
/// `Some`, falling back to a full scan otherwise (tests, mock provider).
///
/// Returns an empty string when no entities are found.
pub async fn fetch_entity_context<C: Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
    collection_ids: &[String],
    query_embedding: Option<&[f32]>,
) -> Result<String, AgentError> {
    // ── Campaign entities (always full scan) ─────────────────────────────────
    // `vault_deleted != true`, never `= false`: DEFAULT does not backfill
    // pre-migration rows, and `= false` would silently drop them. A
    // soft-deleted entity must never be quoted back to the GM in a chat answer.
    let mut resp = db
        .query("SELECT name, summary, notes, player_name, character_class, character_level, status, codex_article FROM player_character WHERE vault_deleted != true AND id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes, codex_article FROM npc WHERE vault_deleted != true AND id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes, codex_article FROM location WHERE vault_deleted != true AND id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes, codex_article FROM faction WHERE vault_deleted != true AND id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes, codex_article FROM creature WHERE vault_deleted != true AND id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes, codex_article FROM item WHERE vault_deleted != true AND id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes, date_start, date_end, codex_article FROM event WHERE vault_deleted != true AND id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes, codex_article FROM misc WHERE vault_deleted != true AND id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT title, notes, date_played, session_number FROM session WHERE vault_deleted != true AND campaign = type::thing('campaign', $cid) ORDER BY session_number ASC")
        .bind(("cid", campaign_id.to_owned()))
        .await
        .map_err(|e| AgentError::Db(e.to_string()))?;

    let pcs: Vec<PcRow> = resp.take(0).map_err(|e| AgentError::Db(e.to_string()))?;
    let npcs: Vec<BasicRow> = resp.take(1).map_err(|e| AgentError::Db(e.to_string()))?;
    let locations: Vec<BasicRow> = resp.take(2).map_err(|e| AgentError::Db(e.to_string()))?;
    let factions: Vec<BasicRow> = resp.take(3).map_err(|e| AgentError::Db(e.to_string()))?;
    let creatures: Vec<BasicRow> = resp.take(4).map_err(|e| AgentError::Db(e.to_string()))?;
    let items: Vec<BasicRow> = resp.take(5).map_err(|e| AgentError::Db(e.to_string()))?;
    let events: Vec<EventRow> = resp.take(6).map_err(|e| AgentError::Db(e.to_string()))?;
    let misc: Vec<BasicRow> = resp.take(7).map_err(|e| AgentError::Db(e.to_string()))?;
    let sessions: Vec<SessionRow> = resp.take(8).map_err(|e| AgentError::Db(e.to_string()))?;

    // ── Collection entities (top-k per table via MTREE, full scan as fallback) ─
    let mut col_entities: Vec<(String, BasicRow)> = Vec::new(); // (kind, row)
    if !collection_ids.is_empty() {
        // Build a WHERE clause that matches entities in any of the given collections.
        // Each `collection:id->in_collection` traversal returns the entity IDs for
        // that collection; OR-ing them covers multiple subscriptions.
        let col_filter: String = collection_ids
            .iter()
            .map(|cid| {
                // Graph-traversal form: from the entity, walk back along the
                // in_collection edge to its collection(s) and test membership.
                // NOTE: a `id IN (SELECT ...)` subquery does NOT compose with the
                // MTREE KNN operator (`embedding <|K|> $vec`) — the combination
                // silently returns zero rows. The traversal form composes; the
                // explicit-array form would too. See the regression test
                // `fetch_entity_context_knn_over_collection_executes`.
                let safe = cid.replace('\'', "\\'");
                format!("<-in_collection<-collection CONTAINS type::thing('collection', '{safe}')")
            })
            .collect::<Vec<_>>()
            .join(" OR ");

        for table in &[
            "npc",
            "location",
            "faction",
            "creature",
            "item",
            "event",
            "player_character",
            "misc",
        ] {
            let sql = if let Some(qv) = query_embedding {
                // MTREE KNN: order by cosine distance, top 10 per table.
                let vec_str = qv
                    .iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                // KNN pattern: the `<|K|>` operator must live in WHERE to activate
                // the index; ordering is by the computed distance.
                // `vault_deleted != true` is a plain field predicate (not an
                // `id IN (SELECT ...)` subquery), which composes fine with the
                // MTREE KNN operator — verified by
                // `fetch_entity_context_knn_over_collection_omits_soft_deleted`.
                format!(
                    "SELECT name, summary, notes, codex_article, vector::distance::knn() AS distance \
                     FROM {table} \
                     WHERE embedding <|10|> [{vec_str}] AND vault_deleted != true AND ({col_filter}) \
                     ORDER BY distance ASC LIMIT 10"
                )
            } else {
                // Full scan fallback (no embedding provider / test paths).
                // `vault_deleted != true`, never `= false` — see module note above.
                format!(
                    "SELECT name, summary, notes, codex_article FROM {table} \
                     WHERE vault_deleted != true AND ({col_filter}) LIMIT 50"
                )
            };
            let mut r = db
                .query(sql)
                .await
                .map_err(|e| AgentError::Db(e.to_string()))?;
            let rows: Vec<BasicRow> = r.take(0).map_err(|e| AgentError::Db(e.to_string()))?;
            for row in rows {
                col_entities.push((table.to_string(), row));
            }
        }
    }

    if pcs.is_empty()
        && npcs.is_empty()
        && locations.is_empty()
        && factions.is_empty()
        && creatures.is_empty()
        && items.is_empty()
        && events.is_empty()
        && misc.is_empty()
        && sessions.is_empty()
        && col_entities.is_empty()
    {
        return Ok(String::new());
    }

    Ok(format_entity_output(
        &pcs,
        &npcs,
        &locations,
        &factions,
        &creatures,
        &items,
        &events,
        &misc,
        &sessions,
        &col_entities,
    ))
}
