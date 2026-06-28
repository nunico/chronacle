use crate::services::entity_service::{EntityInput, EntityKind};
use surrealdb::engine::local::Db;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

pub async fn setup_db() -> Surreal<Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db
}

pub fn make_npc(name: &str) -> EntityInput {
    EntityInput {
        name: name.to_string(),
        summary: None,
        notes: None,
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
pub async fn create_campaign(db: &Surreal<Db>) -> String {
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
pub async fn create_collection(db: &Surreal<Db>) -> String {
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
