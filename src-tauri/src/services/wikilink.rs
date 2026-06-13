use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use surrealdb::sql::Thing;
use thiserror::Error;

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug, Error, Serialize)]
pub enum WikilinkError {
    #[error("Database error: {message}")]
    Database { message: String },
    #[error("Invalid identifier '{value}': only [a-zA-Z0-9_] characters are allowed")]
    InvalidIdentifier { value: String },
    #[error("Malformed record ID '{value}': expected 'table:id'")]
    MalformedRecordId { value: String },
}

// ── Scope ─────────────────────────────────────────────────────────────────────

/// Determines which entities are candidates for wikilink resolution.
pub enum WikilinkScope<'a> {
    /// Source is a campaign entity. Resolves against:
    ///   - all entities in the campaign (`in_campaign` edges), AND
    ///   - all entities in collections the campaign subscribes to
    ///     (`subscribes_to->in_collection` chained traversal).
    Campaign { campaign_id: &'a str },

    /// Source is a collection entity. Resolves against:
    ///   - all entities in the same collection only (`in_collection` edges).
    Collection { collection_id: &'a str },
}

// ── Constants ─────────────────────────────────────────────────────────────────

const ENTITY_TABLES: &[&str] = &[
    "npc",
    "location",
    "faction",
    "creature",
    "item",
    "event",
    "player_character",
    "misc",
];

// Compiled once at first use — avoids recompiling the pattern on every call.
static WIKILINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\[\]]+)\]\]").expect("wikilink regex is valid"));

// ── Internal structs ─────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
struct EntityNameRow {
    id: Thing,
    name: String,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Parse `[[name]]` wikilinks from `notes`, resolve them against all entity
/// tables within `scope`, and (when `source_table` is one of the 8 entity
/// tables) maintain `relates_to` edges with `rel_type = "mentioned"`.
///
/// Returns the full record IDs (e.g. `"npc:abc123"`) of every matched entity.
/// Callers whose source table is *not* an entity table (e.g. `"session"`) must
/// persist the returned IDs themselves.
pub async fn parse_and_sync_wikilinks<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    source_table: &str,
    source_id: &str,
    notes: &str,
    scope: WikilinkScope<'_>,
) -> Result<Vec<String>, WikilinkError> {
    // Guard against injection through the source identifiers.
    validate_identifier(source_table)?;
    validate_identifier(source_id)?;

    // 1. Extract [[name]] patterns
    let extracted: Vec<String> = WIKILINK_RE
        .captures_iter(notes)
        .map(|cap| cap[1].trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if extracted.is_empty() {
        // Still need to delete stale edges for entity sources
        if ENTITY_TABLES.contains(&source_table) {
            delete_stale_mentioned_edges(db, source_table, source_id, &[]).await?;
        }
        return Ok(vec![]);
    }

    // 2. Query all entity names within the scope across all 8 tables.
    let all_entities = query_all_entity_names(db, &scope).await?;

    // 3. Case-insensitive name matching
    let mut matched_ids: Vec<String> = extracted
        .iter()
        .filter_map(|wikilink_name| {
            let lower = wikilink_name.to_lowercase();
            all_entities
                .iter()
                .find(|(_, name)| name.to_lowercase() == lower)
                .map(|(id, _)| id.clone())
        })
        .collect();

    // Deduplicate so that repeated wikilinks to the same entity produce a
    // single edge rather than multiple identical edges.
    matched_ids.sort_unstable();
    matched_ids.dedup();

    // 4 & 5. Write / sync edges only for entity→entity relations
    if ENTITY_TABLES.contains(&source_table) {
        upsert_mentioned_edges(db, source_table, source_id, &matched_ids).await?;
        delete_stale_mentioned_edges(db, source_table, source_id, &matched_ids).await?;
    }

    Ok(matched_ids)
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Reject any string that is empty or contains characters outside `[a-zA-Z0-9_]`.
///
/// Applied to every table name and record ID before they are interpolated into
/// a SurrealQL query string, preventing query-structure injection.
fn validate_identifier(s: &str) -> Result<(), WikilinkError> {
    if !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_') {
        Ok(())
    } else {
        Err(WikilinkError::InvalidIdentifier {
            value: s.to_string(),
        })
    }
}

/// Validate a full record ID such as `"npc:abc123"` by splitting on `:` and
/// validating each component with [`validate_identifier`].
fn validate_record_id(s: &str) -> Result<(), WikilinkError> {
    let (table, id) = split_record_id(s)?;
    validate_identifier(table)?;
    validate_identifier(id)?;
    Ok(())
}

/// Query the `name` and `id` from all 8 entity tables within the given scope.
///
/// **Campaign scope**: entities reachable via `in_campaign` edges from the
/// campaign, OR via chained `subscribes_to->in_collection` traversal (i.e.
/// entities in any collection the campaign subscribes to).
///
/// **Collection scope**: entities reachable via `in_collection` edges from
/// the collection only.
async fn query_all_entity_names<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    scope: &WikilinkScope<'_>,
) -> Result<Vec<(String, String)>, WikilinkError> {
    let mut query = String::new();

    match scope {
        WikilinkScope::Campaign { campaign_id } => {
            for table in ENTITY_TABLES {
                // Entities in the campaign OR in any subscribed collection.
                // `campaign:$cid->subscribes_to->in_collection` chains:
                //   campaign → subscribes_to edge → collection → in_collection edge → entity
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
                .map_err(|e| WikilinkError::Database {
                    message: e.to_string(),
                })?;

            let mut results: Vec<(String, String)> = Vec::new();
            for i in 0..ENTITY_TABLES.len() {
                let rows: Vec<EntityNameRow> =
                    response.take(i).map_err(|e| WikilinkError::Database {
                        message: e.to_string(),
                    })?;
                for row in rows {
                    let record_id = format!("{}:{}", row.id.tb, row.id.id.to_raw());
                    results.push((record_id, row.name));
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
                .map_err(|e| WikilinkError::Database {
                    message: e.to_string(),
                })?;

            let mut results: Vec<(String, String)> = Vec::new();
            for i in 0..ENTITY_TABLES.len() {
                let rows: Vec<EntityNameRow> =
                    response.take(i).map_err(|e| WikilinkError::Database {
                        message: e.to_string(),
                    })?;
                for row in rows {
                    let record_id = format!("{}:{}", row.id.tb, row.id.id.to_raw());
                    results.push((record_id, row.name));
                }
            }
            Ok(results)
        }
    }
}

/// Upsert `relates_to` edges from `source_table:source_id` to each of
/// `matched_ids` with `rel_type = "mentioned"`.
///
/// SurrealDB `RELATE` does not deduplicate — it always creates a new edge.
/// To prevent duplicate edges on repeated calls, each target edge is explicitly
/// deleted before being re-created.
async fn upsert_mentioned_edges<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    source_table: &str,
    source_id: &str,
    matched_ids: &[String],
) -> Result<(), WikilinkError> {
    for target_full_id in matched_ids {
        // target_full_id is e.g. "npc:abc123" — split at the first colon
        let (to_table, to_id) = split_record_id(target_full_id)?;

        // Validate target components before interpolation.
        validate_identifier(to_table)?;
        validate_identifier(to_id)?;

        // Delete any pre-existing edge between this exact source and target so
        // that RELATE does not create a second (duplicate) edge.
        let delete_query = format!(
            "DELETE relates_to \
             WHERE in = {source_table}:{source_id} \
             AND out = {to_table}:{to_id} \
             AND rel_type = 'mentioned'"
        );
        db.query(delete_query)
            .await
            .map_err(|e| WikilinkError::Database {
                message: e.to_string(),
            })?;

        // Use format! for the record ID syntax on both sides of the arrow,
        // matching the pattern in entity_service::relate.
        let relate_query = format!(
            "RELATE {source_table}:{source_id}->relates_to->{to_table}:{to_id} \
             SET rel_type = 'mentioned', notes = NULL, created_at = time::now()"
        );
        db.query(relate_query)
            .await
            .map_err(|e| WikilinkError::Database {
                message: e.to_string(),
            })?;
    }
    Ok(())
}

/// Delete stale `relates_to` edges where the source is this record,
/// `rel_type = "mentioned"`, and the target is NOT in `keep_ids`.
async fn delete_stale_mentioned_edges<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    source_table: &str,
    source_id: &str,
    keep_ids: &[String],
) -> Result<(), WikilinkError> {
    if keep_ids.is_empty() {
        // Delete all "mentioned" edges from this source — no exclusion needed
        let query = format!(
            "DELETE relates_to \
             WHERE in = {source_table}:{source_id} \
             AND rel_type = 'mentioned'"
        );
        db.query(query).await.map_err(|e| WikilinkError::Database {
            message: e.to_string(),
        })?;
    } else {
        // Validate each keep ID before interpolating into the query string.
        for id in keep_ids {
            validate_record_id(id)?;
        }

        // Build the NOT IN list as literal record IDs in the query string
        let keep_list = keep_ids.join(", ");
        let query = format!(
            "DELETE relates_to \
             WHERE in = {source_table}:{source_id} \
             AND rel_type = 'mentioned' \
             AND out NOT IN [{keep_list}]"
        );
        db.query(query).await.map_err(|e| WikilinkError::Database {
            message: e.to_string(),
        })?;
    }

    Ok(())
}

/// Split a full record ID like `"npc:abc123"` into `("npc", "abc123")`.
fn split_record_id(full_id: &str) -> Result<(&str, &str), WikilinkError> {
    let pos = full_id
        .find(':')
        .ok_or_else(|| WikilinkError::MalformedRecordId {
            value: full_id.to_string(),
        })?;
    Ok((&full_id[..pos], &full_id[pos + 1..]))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::entity_service::{create, EntityInput, EntityKind};
    use surrealdb::engine::local::Db;
    use surrealdb::Surreal;

    async fn setup_db() -> Surreal<Db> {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db
    }

    fn make_npc(name: &str) -> EntityInput {
        EntityInput {
            name: name.to_string(),
            summary: None,
            notes: None,
            is_gm_only: None,
            date_start: None,
            date_end: None,
            is_ongoing: None,
            sequence_index: None,
            era: None,
            duration_label: None,
            session_id: None,
            player_name: None,
            character_class: None,
            character_level: None,
            status: None,
        }
    }

    /// Helper: create a campaign and return its ID.
    async fn create_campaign(db: &Surreal<Db>) -> String {
        #[derive(serde::Deserialize)]
        struct Row {
            id: Thing,
        }
        let mut resp = db
            .query(
                "CREATE campaign SET \
                 name = 'Test Campaign', \
                 system = 'D&D 5e', \
                 created_at = time::now(), \
                 updated_at = time::now()",
            )
            .await
            .unwrap();
        let rows: Vec<Row> = resp.take(0).unwrap();
        rows.into_iter().next().unwrap().id.id.to_raw()
    }

    /// Helper: create a collection and return its ID.
    async fn create_collection(db: &Surreal<Db>) -> String {
        #[derive(serde::Deserialize)]
        struct Row {
            id: Thing,
        }
        let mut resp = db
            .query(
                "CREATE collection SET \
                 name = 'Test Collection', \
                 description = NULL, \
                 created_at = time::now(), \
                 updated_at = time::now()",
            )
            .await
            .unwrap();
        let rows: Vec<Row> = resp.take(0).unwrap();
        rows.into_iter().next().unwrap().id.id.to_raw()
    }

    // ── Test 1: empty notes ──────────────────────────────────────────────────

    #[tokio::test]
    async fn empty_notes_returns_empty_vec_no_db_changes() {
        let db = setup_db().await;
        let campaign_id = create_campaign(&db).await;

        let result = parse_and_sync_wikilinks(
            &db,
            "npc",
            "someId",
            "",
            WikilinkScope::Campaign {
                campaign_id: &campaign_id,
            },
        )
        .await
        .unwrap();

        assert!(result.is_empty());

        // No relates_to edges should exist
        let mut resp = db
            .query("SELECT count() FROM relates_to GROUP ALL")
            .await
            .unwrap();
        let count: Option<serde_json::Value> = resp.take(0).unwrap();
        let n = count
            .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
            .unwrap_or(0);
        assert_eq!(n, 0, "no relates_to edges should exist");
    }

    // ── Test 2: nonexistent name → empty Vec ─────────────────────────────────

    #[tokio::test]
    async fn nonexistent_wikilink_returns_empty_vec() {
        let db = setup_db().await;
        let campaign_id = create_campaign(&db).await;

        let result = parse_and_sync_wikilinks(
            &db,
            "npc",
            "someId",
            "[[NonExistentName]]",
            WikilinkScope::Campaign {
                campaign_id: &campaign_id,
            },
        )
        .await
        .unwrap();

        assert!(result.is_empty());
    }

    // ── Test 3: case-insensitive match ───────────────────────────────────────

    #[tokio::test]
    async fn case_insensitive_match_returns_entity_id() {
        let db = setup_db().await;
        let campaign_id = create_campaign(&db).await;

        let npc = create(
            &db,
            Some(&campaign_id),
            None,
            EntityKind::Npc,
            make_npc("torvin"),
        )
        .await
        .unwrap();
        let expected_id = format!("npc:{}", npc.id);

        let source_npc = create(
            &db,
            Some(&campaign_id),
            None,
            EntityKind::Npc,
            make_npc("SourceNPC"),
        )
        .await
        .unwrap();

        let result = parse_and_sync_wikilinks(
            &db,
            "npc",
            &source_npc.id,
            "We met [[Torvin]] at the inn.",
            WikilinkScope::Campaign {
                campaign_id: &campaign_id,
            },
        )
        .await
        .unwrap();

        assert_eq!(result, vec![expected_id]);
    }

    // ── Test 4: stale edge deleted on second call ────────────────────────────

    #[tokio::test]
    async fn stale_relates_to_edge_deleted_on_second_call() {
        let db = setup_db().await;
        let campaign_id = create_campaign(&db).await;

        let torvin = create(
            &db,
            Some(&campaign_id),
            None,
            EntityKind::Npc,
            make_npc("Torvin"),
        )
        .await
        .unwrap();
        let source_npc = create(
            &db,
            Some(&campaign_id),
            None,
            EntityKind::Npc,
            make_npc("SourceNPC"),
        )
        .await
        .unwrap();

        parse_and_sync_wikilinks(
            &db,
            "npc",
            &source_npc.id,
            "We met [[Torvin]] at the inn.",
            WikilinkScope::Campaign {
                campaign_id: &campaign_id,
            },
        )
        .await
        .unwrap();

        let mut resp = db
            .query("SELECT count() FROM relates_to GROUP ALL")
            .await
            .unwrap();
        let count: Option<serde_json::Value> = resp.take(0).unwrap();
        let n = count
            .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
            .unwrap_or(0);
        assert_eq!(n, 1, "edge should exist after first call");

        let result2 = parse_and_sync_wikilinks(
            &db,
            "npc",
            &source_npc.id,
            "The inn was empty.",
            WikilinkScope::Campaign {
                campaign_id: &campaign_id,
            },
        )
        .await
        .unwrap();

        assert!(result2.is_empty());

        let mut resp2 = db
            .query("SELECT count() FROM relates_to GROUP ALL")
            .await
            .unwrap();
        let count2: Option<serde_json::Value> = resp2.take(0).unwrap();
        let n2 = count2
            .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
            .unwrap_or(0);
        assert_eq!(n2, 0, "stale edge should be deleted after second call");

        let _ = torvin;
    }

    // ── Test 5: multiple matches ─────────────────────────────────────────────

    #[tokio::test]
    async fn multiple_wikilinks_all_returned() {
        let db = setup_db().await;
        let campaign_id = create_campaign(&db).await;

        let torvin = create(
            &db,
            Some(&campaign_id),
            None,
            EntityKind::Npc,
            make_npc("Torvin"),
        )
        .await
        .unwrap();
        let ironhold = create(
            &db,
            Some(&campaign_id),
            None,
            EntityKind::Location,
            make_npc("Ironhold"),
        )
        .await
        .unwrap();

        let source_npc = create(
            &db,
            Some(&campaign_id),
            None,
            EntityKind::Npc,
            make_npc("SourceNPC"),
        )
        .await
        .unwrap();

        let result = parse_and_sync_wikilinks(
            &db,
            "npc",
            &source_npc.id,
            "[[Torvin]] traveled to [[Ironhold]] yesterday.",
            WikilinkScope::Campaign {
                campaign_id: &campaign_id,
            },
        )
        .await
        .unwrap();

        assert_eq!(result.len(), 2);
        assert!(
            result.contains(&format!("npc:{}", torvin.id)),
            "should contain Torvin"
        );
        assert!(
            result.contains(&format!("location:{}", ironhold.id)),
            "should contain Ironhold"
        );
    }

    // ── Test 6: session source skips DB edges but still returns matched IDs ──

    #[tokio::test]
    async fn session_source_skips_relates_to_edges_but_returns_ids() {
        let db = setup_db().await;
        let campaign_id = create_campaign(&db).await;

        let torvin = create(
            &db,
            Some(&campaign_id),
            None,
            EntityKind::Npc,
            make_npc("Torvin"),
        )
        .await
        .unwrap();

        let result = parse_and_sync_wikilinks(
            &db,
            "session",
            "somesessionid",
            "[[Torvin]] appeared.",
            WikilinkScope::Campaign {
                campaign_id: &campaign_id,
            },
        )
        .await
        .unwrap();

        assert_eq!(result, vec![format!("npc:{}", torvin.id)]);

        let mut resp = db
            .query("SELECT count() FROM relates_to GROUP ALL")
            .await
            .unwrap();
        let count: Option<serde_json::Value> = resp.take(0).unwrap();
        let n = count
            .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
            .unwrap_or(0);
        assert_eq!(n, 0, "session source must not create relates_to edges");
    }

    // ── Test 7: duplicate edges regression ───────────────────────────────────

    #[tokio::test]
    async fn repeated_call_same_notes_produces_single_edge() {
        let db = setup_db().await;
        let campaign_id = create_campaign(&db).await;

        let torvin = create(
            &db,
            Some(&campaign_id),
            None,
            EntityKind::Npc,
            make_npc("Torvin"),
        )
        .await
        .unwrap();
        let source_npc = create(
            &db,
            Some(&campaign_id),
            None,
            EntityKind::Npc,
            make_npc("SourceNPC"),
        )
        .await
        .unwrap();

        let notes = "We met [[Torvin]] at the inn.";

        parse_and_sync_wikilinks(
            &db,
            "npc",
            &source_npc.id,
            notes,
            WikilinkScope::Campaign {
                campaign_id: &campaign_id,
            },
        )
        .await
        .unwrap();
        parse_and_sync_wikilinks(
            &db,
            "npc",
            &source_npc.id,
            notes,
            WikilinkScope::Campaign {
                campaign_id: &campaign_id,
            },
        )
        .await
        .unwrap();

        let source_record = format!("npc:{}", source_npc.id);
        let mut resp = db
            .query(format!(
                "SELECT count() FROM relates_to \
                 WHERE in = {source_record} AND rel_type = 'mentioned' \
                 GROUP ALL"
            ))
            .await
            .unwrap();
        let count: Option<serde_json::Value> = resp.take(0).unwrap();
        let n = count
            .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
            .unwrap_or(0);
        assert_eq!(
            n, 1,
            "repeated calls with same notes must produce exactly one edge, got {n}"
        );

        let _ = torvin;
    }

    // ── Test 8: validate_identifier rejects injection strings ─────────────────

    #[test]
    fn validate_identifier_rejects_special_chars() {
        assert!(validate_identifier("npc").is_ok());
        assert!(validate_identifier("player_character").is_ok());
        assert!(validate_identifier("abc123").is_ok());

        assert!(validate_identifier("npc; DROP TABLE npc").is_err());
        assert!(validate_identifier("npc->relates_to").is_err());
        assert!(validate_identifier("foo:bar").is_err());
        assert!(validate_identifier("").is_err());
    }

    // ── Test 9: invalid source_table returns error ────────────────────────────

    #[tokio::test]
    async fn invalid_source_table_returns_error() {
        let db = setup_db().await;
        let campaign_id = create_campaign(&db).await;

        let result = parse_and_sync_wikilinks(
            &db,
            "npc; DROP TABLE npc",
            "someId",
            "some notes",
            WikilinkScope::Campaign {
                campaign_id: &campaign_id,
            },
        )
        .await;

        assert!(
            matches!(result, Err(WikilinkError::InvalidIdentifier { .. })),
            "expected InvalidIdentifier error, got {result:?}"
        );
    }

    // ── Test 10: split_record_id returns error on missing colon ───────────────

    #[test]
    fn split_record_id_error_on_missing_colon() {
        let result = split_record_id("npcabc123");
        assert!(
            matches!(result, Err(WikilinkError::MalformedRecordId { .. })),
            "expected MalformedRecordId, got {result:?}"
        );

        let ok = split_record_id("npc:abc123");
        assert!(ok.is_ok());
        let (table, id) = ok.unwrap();
        assert_eq!(table, "npc");
        assert_eq!(id, "abc123");
    }

    // ── Test 11: ENTITY_TABLES drift guard ────────────────────────────────────

    #[test]
    fn entity_tables_matches_entity_kind() {
        use crate::services::entity_service::EntityKind;

        for t in ENTITY_TABLES {
            EntityKind::from_table(t)
                .unwrap_or_else(|_| panic!("ENTITY_TABLES entry '{t}' not in EntityKind"));
        }

        let kind_count = [
            EntityKind::Npc,
            EntityKind::Location,
            EntityKind::Faction,
            EntityKind::Creature,
            EntityKind::Item,
            EntityKind::Event,
            EntityKind::PlayerCharacter,
            EntityKind::Misc,
        ]
        .len();
        assert_eq!(
            ENTITY_TABLES.len(),
            kind_count,
            "ENTITY_TABLES length doesn't match EntityKind variant count"
        );
    }

    // ── Test 12: duplicate wikilinks produce a single edge ────────────────────

    #[tokio::test]
    async fn duplicate_wikilink_in_notes_produces_single_edge() {
        let db = setup_db().await;
        let campaign_id = create_campaign(&db).await;

        let torvin = create(
            &db,
            Some(&campaign_id),
            None,
            EntityKind::Npc,
            make_npc("Torvin"),
        )
        .await
        .unwrap();
        let source_npc = create(
            &db,
            Some(&campaign_id),
            None,
            EntityKind::Npc,
            make_npc("SourceNPC"),
        )
        .await
        .unwrap();

        parse_and_sync_wikilinks(
            &db,
            "npc",
            &source_npc.id,
            "[[Torvin]] met [[Torvin]] again",
            WikilinkScope::Campaign {
                campaign_id: &campaign_id,
            },
        )
        .await
        .unwrap();

        let source_record = format!("npc:{}", source_npc.id);
        let torvin_record = format!("npc:{}", torvin.id);
        let mut resp = db
            .query(format!(
                "SELECT count() FROM relates_to \
                 WHERE in = {source_record} AND out = {torvin_record} \
                 AND rel_type = 'mentioned' GROUP ALL"
            ))
            .await
            .unwrap();
        let count: Option<serde_json::Value> = resp.take(0).unwrap();
        let n = count
            .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
            .unwrap_or(0);
        assert_eq!(
            n, 1,
            "duplicate wikilinks must produce exactly one edge, got {n}"
        );
    }

    // ── Test 13: collection scope only resolves same-collection entities ───────

    #[tokio::test]
    async fn collection_scope_resolves_same_collection_entities() {
        let db = setup_db().await;
        let col_id = create_collection(&db).await;

        let npc = create(
            &db,
            None,
            Some(&col_id),
            EntityKind::Npc,
            make_npc("Goblin"),
        )
        .await
        .unwrap();
        let expected_id = format!("npc:{}", npc.id);

        // A campaign entity with the same name — must NOT match under collection scope
        let campaign_id = create_campaign(&db).await;
        create(
            &db,
            Some(&campaign_id),
            None,
            EntityKind::Npc,
            make_npc("Goblin"),
        )
        .await
        .unwrap();

        let source_npc = create(
            &db,
            None,
            Some(&col_id),
            EntityKind::Npc,
            make_npc("SourceNPC"),
        )
        .await
        .unwrap();

        let result = parse_and_sync_wikilinks(
            &db,
            "npc",
            &source_npc.id,
            "We fought [[Goblin]].",
            WikilinkScope::Collection {
                collection_id: &col_id,
            },
        )
        .await
        .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0], expected_id,
            "should match collection entity, not campaign entity"
        );
    }

    // ── Test 14: campaign scope resolves subscribed collection entities ────────

    #[tokio::test]
    async fn campaign_scope_resolves_subscribed_collection_entities() {
        let db = setup_db().await;
        let campaign_id = create_campaign(&db).await;
        let col_id = create_collection(&db).await;

        // Subscribe campaign to collection
        db.query(
            "LET $in  = type::thing('campaign',   $cid); \
             LET $out = type::thing('collection', $colid); \
             RELATE $in->subscribes_to->$out SET created_at = time::now()",
        )
        .bind(("cid", campaign_id.clone()))
        .bind(("colid", col_id.clone()))
        .await
        .unwrap();

        // Create collection entity
        let col_npc = create(
            &db,
            None,
            Some(&col_id),
            EntityKind::Npc,
            make_npc("Dungeon Master"),
        )
        .await
        .unwrap();
        let expected_id = format!("npc:{}", col_npc.id);

        // Create campaign entity source
        let source = create(
            &db,
            Some(&campaign_id),
            None,
            EntityKind::Npc,
            make_npc("Player"),
        )
        .await
        .unwrap();

        let result = parse_and_sync_wikilinks(
            &db,
            "npc",
            &source.id,
            "Asked the [[Dungeon Master]] for help.",
            WikilinkScope::Campaign {
                campaign_id: &campaign_id,
            },
        )
        .await
        .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], expected_id);
    }
}
