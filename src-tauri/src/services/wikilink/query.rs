use super::{EntityNameRow, EntityNotesRow, WikilinkError, WikilinkScope, ENTITY_TABLES};

/// Query the `name` and `id` from all 8 entity tables within the given scope.
///
/// **Campaign scope**: entities reachable via `in_campaign` edges from the
/// campaign, OR via chained `subscribes_to->in_collection` traversal.
///
/// **Collection scope**: entities reachable via `in_collection` edges from
/// the collection only.
pub(super) async fn query_all_entity_names<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    scope: &WikilinkScope<'_>,
) -> Result<Vec<(String, String)>, WikilinkError> {
    let mut query = String::new();

    match scope {
        WikilinkScope::Campaign { campaign_id } => {
            for table in ENTITY_TABLES {
                query.push_str(&format!(
                    "SELECT id, name FROM {table} \
                     WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $campaign_id)) \
                        OR id IN (SELECT VALUE out FROM in_collection \
                                  WHERE in IN (SELECT VALUE out FROM subscribes_to \
                                               WHERE in = type::thing('campaign', $campaign_id)));"
                ));
            }
            let mut response = db
                .query(query)
                .bind(("campaign_id", (*campaign_id).to_owned()))
                .await
                .map_err(|e| WikilinkError::Database { message: e.to_string() })?;

            let mut results: Vec<(String, String)> = Vec::new();
            for i in 0..ENTITY_TABLES.len() {
                let rows: Vec<EntityNameRow> =
                    response.take(i).map_err(|e| WikilinkError::Database { message: e.to_string() })?;
                for row in rows {
                    results.push((format!("{}:{}", row.id.tb, row.id.id.to_raw()), row.name));
                }
            }
            Ok(results)
        }

        WikilinkScope::Collection { collection_id } => {
            for table in ENTITY_TABLES {
                query.push_str(&format!(
                    "SELECT id, name FROM {table} \
                     WHERE id IN (SELECT VALUE out FROM in_collection WHERE in = type::thing('collection', $collection_id));"
                ));
            }
            let mut response = db
                .query(query)
                .bind(("collection_id", (*collection_id).to_owned()))
                .await
                .map_err(|e| WikilinkError::Database { message: e.to_string() })?;

            let mut results: Vec<(String, String)> = Vec::new();
            for i in 0..ENTITY_TABLES.len() {
                let rows: Vec<EntityNameRow> =
                    response.take(i).map_err(|e| WikilinkError::Database { message: e.to_string() })?;
                for row in rows {
                    results.push((format!("{}:{}", row.id.tb, row.id.id.to_raw()), row.name));
                }
            }
            Ok(results)
        }
    }
}

/// Query `id`, `name`, and `notes` from all 8 entity tables within the given scope.
///
/// Returns a `Vec` of `(full_record_id, name, notes)` triples.
/// Uses the same scope WHERE clauses as [`query_all_entity_names`].
pub(super) async fn query_all_entity_notes<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    scope: &WikilinkScope<'_>,
) -> Result<Vec<(String, String, Option<String>)>, WikilinkError> {
    let mut query = String::new();

    match scope {
        WikilinkScope::Campaign { campaign_id } => {
            for table in ENTITY_TABLES {
                query.push_str(&format!(
                    "SELECT id, name, notes FROM {table} \
                     WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $campaign_id)) \
                        OR id IN (SELECT VALUE out FROM in_collection \
                                  WHERE in IN (SELECT VALUE out FROM subscribes_to \
                                               WHERE in = type::thing('campaign', $campaign_id)));"
                ));
            }
            let mut response = db
                .query(query)
                .bind(("campaign_id", (*campaign_id).to_owned()))
                .await
                .map_err(|e| WikilinkError::Database { message: e.to_string() })?;

            let mut results: Vec<(String, String, Option<String>)> = Vec::new();
            for i in 0..ENTITY_TABLES.len() {
                let rows: Vec<EntityNotesRow> =
                    response.take(i).map_err(|e| WikilinkError::Database { message: e.to_string() })?;
                for row in rows {
                    results.push((format!("{}:{}", row.id.tb, row.id.id.to_raw()), row.name, row.notes));
                }
            }
            Ok(results)
        }

        WikilinkScope::Collection { collection_id } => {
            for table in ENTITY_TABLES {
                query.push_str(&format!(
                    "SELECT id, name, notes FROM {table} \
                     WHERE id IN (SELECT VALUE out FROM in_collection WHERE in = type::thing('collection', $collection_id));"
                ));
            }
            let mut response = db
                .query(query)
                .bind(("collection_id", (*collection_id).to_owned()))
                .await
                .map_err(|e| WikilinkError::Database { message: e.to_string() })?;

            let mut results: Vec<(String, String, Option<String>)> = Vec::new();
            for i in 0..ENTITY_TABLES.len() {
                let rows: Vec<EntityNotesRow> =
                    response.take(i).map_err(|e| WikilinkError::Database { message: e.to_string() })?;
                for row in rows {
                    results.push((format!("{}:{}", row.id.tb, row.id.id.to_raw()), row.name, row.notes));
                }
            }
            Ok(results)
        }
    }
}
