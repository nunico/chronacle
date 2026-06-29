//! Backend E2E (service layer, real in-memory SurrealDB):
//! create a campaign → add an NPC and an event through the real entity service
//! → run the agent's entity-context retrieval → assert both surface in the
//! context the LLM would receive. Drives the service API directly (no IPC, no
//! mock LLM) per ADR-006's "backend E2E" pattern.

use chronacle_domain::campaign_service;
use chronacle_extraction::entity_service::{self, EntityInput, EntityKind};
use chronacle_retrieval::agent_service::fetch_entity_context;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;

async fn setup_db() -> Surreal<Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db
}

#[tokio::test]
async fn campaign_npc_and_event_both_appear_in_agent_context() {
    let db = setup_db().await;

    let campaign = campaign_service::create(&db, "Shadows of Valdris", "D&D 5e")
        .await
        .unwrap();

    // Add an NPC with notes…
    entity_service::create(
        &db,
        Some(&campaign.id),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Seraphina Aldric".to_string(),
            summary: Some("archivist of the Iron Tower".to_string()),
            notes: Some("Secretly guards the Sunstone.".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // …and an event.
    entity_service::create(
        &db,
        Some(&campaign.id),
        None,
        EntityKind::Event,
        EntityInput {
            name: "The Battle of Irongate".to_string(),
            summary: Some("The siege that ended the war".to_string()),
            date_start: Some("Year 312".to_string()),
            sequence_index: Some(1),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // The agent builds entity context for the active campaign. Use the full-scan
    // path (no query embedding) so the assertion is deterministic.
    let context = fetch_entity_context(&db, &campaign.id, &[], None)
        .await
        .unwrap();

    assert!(
        context.contains("[npc] Seraphina Aldric"),
        "NPC missing from agent context: {context}"
    );
    assert!(
        context.contains("Secretly guards the Sunstone."),
        "NPC notes missing from agent context: {context}"
    );
    assert!(
        context.contains("[event] The Battle of Irongate"),
        "event missing from agent context: {context}"
    );

    // A second campaign must not see the first campaign's entities.
    let other = campaign_service::create(&db, "Unrelated", "PF2e")
        .await
        .unwrap();
    let empty = fetch_entity_context(&db, &other.id, &[], None)
        .await
        .unwrap();
    assert!(
        empty.is_empty(),
        "entities leaked into an unrelated campaign: {empty}"
    );
}
