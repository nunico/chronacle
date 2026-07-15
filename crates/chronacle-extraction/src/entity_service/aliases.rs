//! Alternate-name management. An alias must be unambiguous WITHIN ITS SCOPE, or
//! tier-2 resolution stops being deterministic and the same link resolves
//! differently depending on row order.

use super::EntityError;
use crate::naming::normalize;
use crate::wikilink::{query_all_entity_names, WikilinkScope};

/// Append an alias, refusing one that collides with another entity's name or
/// alias in the same resolution scope. The collision is a REFUSAL, not a
/// silent skip — the caller (and the GM) must know it did not take.
pub async fn add_alias<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    full_id: &str,
    alias: &str,
    scope: WikilinkScope<'_>,
) -> Result<(), EntityError> {
    let alias = alias.trim();
    if alias.is_empty() {
        return Err(EntityError::Validation {
            field: "alias".into(),
            message: "An alternate name cannot be empty".into(),
        });
    }

    let norm = normalize(alias);
    let entities = query_all_entity_names(db, &scope)
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;

    if let Some(other) = entities.iter().find(|e| {
        e.id != full_id
            && (normalize(&e.name) == norm || e.aliases.iter().any(|a| normalize(a) == norm))
    }) {
        return Err(EntityError::Validation {
            field: "alias".into(),
            message: format!("\"{alias}\" is already used by {}", other.name),
        });
    }

    let (table, id) = full_id
        .split_once(':')
        .ok_or_else(|| EntityError::Validation {
            field: "id".into(),
            message: format!("Malformed record id: {full_id}"),
        })?;
    // `array::union` (already used elsewhere in this codebase for the same
    // purpose, e.g. `rules.rs`'s `page_refs`) makes the write idempotent:
    // calling `add_alias` twice with the same (entity, alias) — e.g. a
    // retried fuzzy-resolve pass — must not store a duplicate entry.
    db.query(format!(
        "UPDATE type::thing('{table}', $id) \
         SET aliases = array::union(aliases, [$alias]), updated_at = time::now()"
    ))
    .bind(("id", id.to_owned()))
    .bind(("alias", alias.to_owned()))
    .await
    .map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?
    .check()
    .map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    Ok(())
}

/// Remove an alias. Unlike [`add_alias`], no collision check is needed —
/// removing a name can never create an ambiguity, only resolve one.
pub async fn remove_alias<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    full_id: &str,
    alias: &str,
) -> Result<(), EntityError> {
    let alias = alias.trim();
    let (table, id) = full_id
        .split_once(':')
        .ok_or_else(|| EntityError::Validation {
            field: "id".into(),
            message: format!("Malformed record id: {full_id}"),
        })?;
    db.query(format!(
        "UPDATE type::thing('{table}', $id) SET aliases -= $alias, updated_at = time::now()"
    ))
    .bind(("id", id.to_owned()))
    .bind(("alias", alias.to_owned()))
    .await
    .map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?
    .check()
    .map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity_service::{create, EntityInput, EntityKind};

    async fn setup_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        chronacle_db::run_migrations(&db).await.unwrap();
        db
    }

    async fn create_campaign(db: &surrealdb::Surreal<surrealdb::engine::local::Db>) -> String {
        #[derive(serde::Deserialize)]
        struct Row {
            id: surrealdb::sql::Thing,
        }
        let mut resp = db
            .query(
                "CREATE campaign SET name = 'Test', system = '5e', \
                 created_at = time::now(), updated_at = time::now()",
            )
            .await
            .unwrap();
        let rows: Vec<Row> = resp.take(0).unwrap();
        rows.into_iter().next().unwrap().id.id.to_raw()
    }

    #[tokio::test]
    async fn add_alias_appends_and_round_trips() {
        let db = setup_db().await;
        let cid = create_campaign(&db).await;
        let node = create(
            &db,
            Some(&cid),
            None,
            EntityKind::Faction,
            EntityInput {
                name: "The Quassar Family".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let full_id = format!("faction:{}", node.id);

        add_alias(
            &db,
            &full_id,
            "The Quassars",
            WikilinkScope::Campaign { campaign_id: &cid },
        )
        .await
        .unwrap();

        let read = crate::entity_service::get_by_id(&db, &node.id, EntityKind::Faction)
            .await
            .unwrap();
        assert_eq!(read.aliases, vec!["The Quassars"]);
    }

    /// A repeated `add_alias` for the same (entity, alias) — e.g. a fuzzy
    /// pass that retries after an earlier partial failure — must be a no-op,
    /// not a duplicate entry in `aliases`.
    #[tokio::test]
    async fn add_alias_is_idempotent_when_called_twice_with_the_same_alias() {
        let db = setup_db().await;
        let cid = create_campaign(&db).await;
        let node = create(
            &db,
            Some(&cid),
            None,
            EntityKind::Faction,
            EntityInput {
                name: "The Quassar Family".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let full_id = format!("faction:{}", node.id);

        add_alias(
            &db,
            &full_id,
            "The Quassars",
            WikilinkScope::Campaign { campaign_id: &cid },
        )
        .await
        .unwrap();
        add_alias(
            &db,
            &full_id,
            "The Quassars",
            WikilinkScope::Campaign { campaign_id: &cid },
        )
        .await
        .unwrap();

        let read = crate::entity_service::get_by_id(&db, &node.id, EntityKind::Faction)
            .await
            .unwrap();
        assert_eq!(
            read.aliases,
            vec!["The Quassars"],
            "a repeated add_alias must not store a duplicate"
        );
    }

    #[tokio::test]
    async fn add_alias_refuses_a_collision_with_another_entitys_name() {
        let db = setup_db().await;
        let cid = create_campaign(&db).await;
        let family = create(
            &db,
            Some(&cid),
            None,
            EntityKind::Faction,
            EntityInput {
                name: "The Quassar Family".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        create(
            &db,
            Some(&cid),
            None,
            EntityKind::Faction,
            EntityInput {
                name: "The Quassars".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let full_id = format!("faction:{}", family.id);

        let err = add_alias(
            &db,
            &full_id,
            "The Quassars",
            WikilinkScope::Campaign { campaign_id: &cid },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, EntityError::Validation { field, .. } if field == "alias"));
    }

    #[tokio::test]
    async fn add_alias_refuses_a_collision_with_another_entitys_alias() {
        let db = setup_db().await;
        let cid = create_campaign(&db).await;
        let family = create(
            &db,
            Some(&cid),
            None,
            EntityKind::Faction,
            EntityInput {
                name: "The Quassar Family".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        create(
            &db,
            Some(&cid),
            None,
            EntityKind::Faction,
            EntityInput {
                name: "The Cartel".to_string(),
                aliases: Some(vec!["The Quassars".to_string()]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let full_id = format!("faction:{}", family.id);

        let err = add_alias(
            &db,
            &full_id,
            "The Quassars",
            WikilinkScope::Campaign { campaign_id: &cid },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, EntityError::Validation { field, .. } if field == "alias"));
    }

    #[tokio::test]
    async fn add_alias_rejects_empty_input() {
        let db = setup_db().await;
        let cid = create_campaign(&db).await;
        let node = create(
            &db,
            Some(&cid),
            None,
            EntityKind::Faction,
            EntityInput {
                name: "The Quassar Family".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let full_id = format!("faction:{}", node.id);

        let err = add_alias(
            &db,
            &full_id,
            "   ",
            WikilinkScope::Campaign { campaign_id: &cid },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, EntityError::Validation { field, .. } if field == "alias"));
    }

    #[tokio::test]
    async fn remove_alias_round_trips() {
        let db = setup_db().await;
        let cid = create_campaign(&db).await;
        let node = create(
            &db,
            Some(&cid),
            None,
            EntityKind::Faction,
            EntityInput {
                name: "The Quassar Family".to_string(),
                aliases: Some(vec!["The Quassars".to_string()]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let full_id = format!("faction:{}", node.id);

        remove_alias(&db, &full_id, "The Quassars").await.unwrap();

        let read = crate::entity_service::get_by_id(&db, &node.id, EntityKind::Faction)
            .await
            .unwrap();
        assert!(read.aliases.is_empty());
    }
}
