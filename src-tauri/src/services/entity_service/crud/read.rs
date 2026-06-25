use serde::Deserialize;

use super::super::{EntityError, EntityKind, GraphNode, GraphNodeRecord, SELECT_SCOPE_ALIASES};

/// Fetch a single node by its raw ID and kind.
pub async fn get_by_id<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    id: &str,
    kind: EntityKind,
) -> Result<GraphNode, EntityError> {
    let table = kind.table_name();
    let sql = format!("SELECT *, {SELECT_SCOPE_ALIASES} FROM type::thing($table, $id)");
    let mut response = db
        .query(sql)
        .bind(("table", table))
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| EntityError::Database { message: e.to_string() })?;
    let records: Vec<GraphNodeRecord> = response.take(0)
        .map_err(|e| EntityError::Database { message: e.to_string() })?;
    records
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| EntityError::NotFound { id: id.to_string() })
}

/// List all nodes of a kind visible to a campaign, ordered by name.
///
/// Returns both campaign-scoped entities (`in_campaign` edge) and entities
/// belonging to any collection the campaign `subscribes_to` (`in_collection`
/// edge). Extraction writes collection-scoped entities, so the campaign browser
/// must include subscribed collections or extracted entities never surface.
pub async fn get_by_campaign<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
    kind: EntityKind,
) -> Result<Vec<GraphNode>, EntityError> {
    let table = kind.table_name();
    let sql = format!(
        "SELECT *, {SELECT_SCOPE_ALIASES} \
         FROM type::table($table) \
         WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $campaign_id)) \
            OR id IN (SELECT VALUE out FROM in_collection \
                      WHERE in IN (SELECT VALUE out FROM subscribes_to \
                                   WHERE in = type::thing('campaign', $campaign_id))) \
         ORDER BY name ASC"
    );
    let mut response = db
        .query(sql)
        .bind(("table", table))
        .bind(("campaign_id", campaign_id.to_owned()))
        .await
        .map_err(|e| EntityError::Database { message: e.to_string() })?;
    let records: Vec<GraphNodeRecord> = response.take(0)
        .map_err(|e| EntityError::Database { message: e.to_string() })?;
    Ok(records.into_iter().map(Into::into).collect())
}

/// Order events for a timeline by their canonical key.
///
/// `sequence_index` is the ordering key (lower = earlier); unsequenced events
/// (`NULL`) sort last. Ties are broken by name for a stable order.
pub fn order_events_for_timeline(mut events: Vec<GraphNode>) -> Vec<GraphNode> {
    use std::cmp::Ordering;
    events.sort_by(|a, b| match (a.sequence_index, b.sequence_index) {
        (Some(x), Some(y)) => x.cmp(&y).then_with(|| a.name.cmp(&b.name)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.name.cmp(&b.name),
    });
    events
}

/// Fetch a campaign's events in timeline order (see [`order_events_for_timeline`]).
pub async fn get_events_timeline<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
) -> Result<Vec<GraphNode>, EntityError> {
    let events = get_by_campaign(db, campaign_id, EntityKind::Event).await?;
    Ok(order_events_for_timeline(events))
}

/// Count entities of every kind that belong to a campaign.
///
/// Returns a map keyed by table name (`npc`, `location`, …) with an entry for
/// all eight kinds, zero included.
pub async fn count_by_campaign<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
) -> Result<std::collections::HashMap<String, u64>, EntityError> {
    #[derive(Deserialize)]
    struct CountRow { c: u64 }

    const ALL_KINDS: [EntityKind; 8] = [
        EntityKind::Npc, EntityKind::Location, EntityKind::Faction, EntityKind::Creature,
        EntityKind::Item, EntityKind::Event, EntityKind::PlayerCharacter, EntityKind::Misc,
    ];

    let mut counts = std::collections::HashMap::new();
    for kind in ALL_KINDS {
        let table = kind.table_name();
        let row: Option<CountRow> = db
            .query(
                "SELECT count() AS c FROM type::table($table) \
                 WHERE id IN (SELECT VALUE out FROM in_campaign \
                              WHERE in = type::thing('campaign', $campaign_id)) \
                    OR id IN (SELECT VALUE out FROM in_collection \
                              WHERE in IN (SELECT VALUE out FROM subscribes_to \
                                           WHERE in = type::thing('campaign', $campaign_id))) \
                 GROUP ALL",
            )
            .bind(("table", table))
            .bind(("campaign_id", campaign_id.to_owned()))
            .await
            .map_err(|e| EntityError::Database { message: e.to_string() })?
            .take(0)
            .map_err(|e| EntityError::Database { message: e.to_string() })?;
        counts.insert(table.to_string(), row.map(|r| r.c).unwrap_or(0));
    }
    Ok(counts)
}

/// List all nodes of a kind for a collection, ordered by name.
pub async fn get_by_collection<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    collection_id: &str,
    kind: EntityKind,
) -> Result<Vec<GraphNode>, EntityError> {
    let table = kind.table_name();
    let sql = format!(
        "SELECT *, {SELECT_SCOPE_ALIASES} \
         FROM type::table($table) \
         WHERE id IN (SELECT VALUE out FROM in_collection WHERE in = type::thing('collection', $collection_id)) \
         ORDER BY name ASC"
    );
    let mut response = db
        .query(sql)
        .bind(("table", table))
        .bind(("collection_id", collection_id.to_owned()))
        .await
        .map_err(|e| EntityError::Database { message: e.to_string() })?;
    let records: Vec<GraphNodeRecord> = response.take(0)
        .map_err(|e| EntityError::Database { message: e.to_string() })?;
    Ok(records.into_iter().map(Into::into).collect())
}

/// Find a collection-scoped entity by name (case-insensitive) and kind.
///
/// Used by `ExtractionService` for deduplication before creating new entities.
pub async fn find_by_name_and_collection<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    collection_id: &str,
    name: &str,
    kind: EntityKind,
) -> Result<Option<GraphNode>, EntityError> {
    let table = kind.table_name();
    let sql = format!(
        "SELECT *, {SELECT_SCOPE_ALIASES} \
         FROM type::table($table) \
         WHERE id IN (SELECT VALUE out FROM in_collection WHERE in = type::thing('collection', $collection_id)) \
             AND string::lowercase(name) = string::lowercase($name) \
         LIMIT 1"
    );
    let mut response = db
        .query(sql)
        .bind(("table", table))
        .bind(("collection_id", collection_id.to_owned()))
        .bind(("name", name.to_owned()))
        .await
        .map_err(|e| EntityError::Database { message: e.to_string() })?;
    let records: Vec<GraphNodeRecord> = response.take(0)
        .map_err(|e| EntityError::Database { message: e.to_string() })?;
    Ok(records.into_iter().next().map(Into::into))
}

/// Return all events that reference the given session.
pub async fn get_events_for_session<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    session_id: &str,
) -> Result<Vec<GraphNode>, EntityError> {
    let sql = format!(
        "SELECT *, {SELECT_SCOPE_ALIASES} FROM event \
         WHERE session = type::thing('session', $session_id)"
    );
    let mut response = db
        .query(sql)
        .bind(("session_id", session_id.to_owned()))
        .await
        .map_err(|e| EntityError::Database { message: e.to_string() })?;
    let records: Vec<GraphNodeRecord> = response.take(0)
        .map_err(|e| EntityError::Database { message: e.to_string() })?;
    Ok(records.into_iter().map(Into::into).collect())
}
