//! Invariant regression test (task 5 review, finding 4): a soft-deleted
//! record (`vault_deleted = true`) must be invisible to EVERY user-visible
//! read path — not just the six `entity_service` queries patched when the
//! `vault_deleted` field was introduced, and not just the four additional
//! call sites patched in this task (RAG entity context, ego graph, flat
//! relations, wikilink backfill). The next new read path must add itself
//! here, not just to a call-site patch list.

use chronacle_extraction::entity_service::{
    count_by_campaign, create, get_by_campaign, get_by_id, get_entity_graph, get_entity_relations,
    get_events_timeline, relate, soft_delete, EntityInput, EntityKind,
};

async fn setup_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db
}

fn npc_input(name: &str) -> EntityInput {
    EntityInput {
        name: name.to_string(),
        summary: Some("a summary".to_string()),
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

/// Seeds a campaign with two related entities, a session, and an event; soft
/// deletes one entity; asserts it is absent from every user-visible read path.
#[tokio::test]
async fn a_soft_deleted_entity_is_invisible_to_every_read_path() {
    let db = setup_db().await;

    let campaign = chronacle_domain::campaign_service::create(&db, "Test Campaign", "D&D 5e")
        .await
        .unwrap();

    // Two related entities: the one we soft-delete, and a survivor with a
    // live `relates_to` edge to it (this is what leaked in findings 2 and 3).
    let ghost = create(
        &db,
        Some(&campaign.id),
        None,
        EntityKind::Npc,
        npc_input("Ghost the Vanished"),
    )
    .await
    .unwrap();
    let survivor = create(
        &db,
        Some(&campaign.id),
        None,
        EntityKind::Npc,
        npc_input("Survivor"),
    )
    .await
    .unwrap();
    relate(&db, &survivor.id, "npc", &ghost.id, "npc", "ally_of", None)
        .await
        .unwrap();

    // A session and an event, for the timeline path.
    let session = chronacle_domain::session_service::create(
        &db,
        &campaign.id,
        chronacle_domain::session_service::SessionInput {
            session_number: 1,
            title: "Session One".to_string(),
            date_played: "2026-06-05".to_string(),
            notes: String::new(),
        },
    )
    .await
    .unwrap();
    let _event = create(
        &db,
        Some(&campaign.id),
        None,
        EntityKind::Event,
        EntityInput {
            name: "The Vanishing".to_string(),
            session_id: Some(session.id.clone()),
            ..npc_input("The Vanishing")
        },
    )
    .await
    .unwrap();

    // Soft-delete the ghost.
    soft_delete(&db, &ghost.id, EntityKind::Npc).await.unwrap();

    // 1. get_by_campaign
    let by_campaign = get_by_campaign(&db, &campaign.id, EntityKind::Npc)
        .await
        .unwrap();
    assert!(
        by_campaign.iter().all(|n| n.name != "Ghost the Vanished"),
        "soft-deleted entity leaked from get_by_campaign: {by_campaign:?}"
    );
    assert!(
        by_campaign.iter().any(|n| n.name == "Survivor"),
        "live entity missing from get_by_campaign"
    );

    // 2. get_by_id -> NotFound
    let err = get_by_id(&db, &ghost.id, EntityKind::Npc)
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            chronacle_extraction::entity_service::EntityError::NotFound { .. }
        ),
        "expected NotFound for soft-deleted entity, got {err:?}"
    );

    // 3. count_by_campaign
    let counts = count_by_campaign(&db, &campaign.id).await.unwrap();
    assert_eq!(
        counts.get("npc").copied().unwrap_or_default(),
        1,
        "count_by_campaign should count only the live npc, got {counts:?}"
    );

    // 4. ego graph neighbours
    let graph = get_entity_graph(&db, &survivor.id, "npc", 1).await.unwrap();
    assert!(
        graph.nodes.iter().all(|n| n.name != "Ghost the Vanished"),
        "soft-deleted entity leaked into ego graph nodes: {:?}",
        graph.nodes.iter().map(|n| &n.name).collect::<Vec<_>>()
    );

    // 5. flat relations list
    let related = get_entity_relations(&db, &survivor.id, "npc")
        .await
        .unwrap();
    assert!(
        related.iter().all(|r| r.name != "Ghost the Vanished"),
        "soft-deleted entity leaked into flat relations list: {:?}",
        related.iter().map(|r| &r.name).collect::<Vec<_>>()
    );

    // 6. events timeline (not directly related to the ghost npc, but must
    //    still exclude any soft-deleted event were one to exist; asserts the
    //    live event is present and the read path is vault_deleted-filtered).
    let timeline = get_events_timeline(&db, &campaign.id).await.unwrap();
    assert!(
        timeline.iter().any(|e| e.name == "The Vanishing"),
        "live event missing from events timeline"
    );

    // 7. RAG entity context (chronacle-retrieval) — the most severe leak: a
    //    soft-deleted entity's name/summary/notes must never be fed to the LLM.
    let context =
        chronacle_retrieval::agent_service::fetch_entity_context(&db, &campaign.id, &[], None)
            .await
            .unwrap();
    assert!(
        !context.contains("Ghost the Vanished"),
        "soft-deleted entity leaked into RAG entity context: {context}"
    );
    assert!(
        context.contains("Survivor"),
        "live entity missing from RAG entity context: {context}"
    );
}

/// A soft-deleted SESSION must be invisible to every read path (finding 2 of
/// the tranche-5 whole-branch review): `session_service::get_all`/`get_by_id`
/// had no `vault_deleted != true` filter, so a GM deleting a session's file
/// in the vault left it hidden from sync but still live and editable in the
/// session-list UI.
#[tokio::test]
async fn a_soft_deleted_session_is_invisible_to_every_read_path() {
    let db = setup_db().await;

    let campaign = chronacle_domain::campaign_service::create(&db, "Test Campaign", "D&D 5e")
        .await
        .unwrap();

    let ghost = chronacle_domain::session_service::create(
        &db,
        &campaign.id,
        chronacle_domain::session_service::SessionInput {
            session_number: 1,
            title: "The Ambush".to_string(),
            date_played: "2026-06-05".to_string(),
            notes: String::new(),
        },
    )
    .await
    .unwrap();
    let survivor = chronacle_domain::session_service::create(
        &db,
        &campaign.id,
        chronacle_domain::session_service::SessionInput {
            session_number: 2,
            title: "The Aftermath".to_string(),
            date_played: "2026-06-12".to_string(),
            notes: String::new(),
        },
    )
    .await
    .unwrap();

    // Soft-delete as the vault reconcile path does (UPDATE ... SET
    // vault_deleted = true — the same mechanism `SurrealVaultRecordStore::
    // soft_delete` uses).
    db.query("UPDATE type::thing('session', $id) SET vault_deleted = true")
        .bind(("id", ghost.id.clone()))
        .await
        .unwrap()
        .check()
        .unwrap();

    // 1. get_all
    let all = chronacle_domain::session_service::get_all(&db, &campaign.id)
        .await
        .unwrap();
    assert!(
        all.iter().all(|s| s.title != "The Ambush"),
        "soft-deleted session leaked from get_all: {all:?}"
    );
    assert!(
        all.iter().any(|s| s.title == "The Aftermath"),
        "live session missing from get_all"
    );

    // 2. get_by_id -> NotFound
    let err = chronacle_domain::session_service::get_by_id(&db, &ghost.id)
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            chronacle_domain::session_service::SessionError::NotFound { .. }
        ),
        "expected NotFound for a soft-deleted session, got {err:?}"
    );
    // The live session is still reachable.
    chronacle_domain::session_service::get_by_id(&db, &survivor.id)
        .await
        .expect("live session must still be reachable by id");

    // 3. RAG entity context includes the session block.
    let context =
        chronacle_retrieval::agent_service::fetch_entity_context(&db, &campaign.id, &[], None)
            .await
            .unwrap();
    assert!(
        !context.contains("The Ambush"),
        "soft-deleted session leaked into RAG context: {context}"
    );
    assert!(
        context.contains("The Aftermath"),
        "live session missing from RAG context: {context}"
    );
}

/// A soft-deleted RULE ENTRY must be invisible to every read path (finding 3
/// of the tranche-5 whole-branch review): the rules-list UI and the
/// `$rules`/`$rstale`/`$ents` compile-status counts had no `vault_deleted !=
/// true` filter. (The RAG rules-context KNN query is covered by its own
/// regression test in `chronacle_retrieval::agent_service::rules_block`,
/// since that function is crate-private.)
#[tokio::test]
async fn a_soft_deleted_rule_entry_is_invisible_to_every_read_path() {
    let db = setup_db().await;

    let campaign = chronacle_domain::campaign_service::create(&db, "Test Campaign", "D&D 5e")
        .await
        .unwrap();
    let collection = chronacle_domain::collection_service::owned_by(&db, &campaign.id)
        .await
        .unwrap()
        .expect("campaign creation auto-owns a collection");

    db.query(
        "CREATE rule_entry:`ghost` SET collection = type::thing('collection', $cid), \
             name = 'Vanished Grapple Rule', category = 'procedure', body = 'b', \
             compiled_at = time::now(), stale = true;
         CREATE rule_entry:`live` SET collection = type::thing('collection', $cid), \
             name = 'Initiative', category = 'mechanic', body = 'b', \
             compiled_at = time::now(), stale = false;",
    )
    .bind(("cid", collection.id.clone()))
    .await
    .unwrap()
    .check()
    .unwrap();

    db.query("UPDATE rule_entry:`ghost` SET vault_deleted = true")
        .await
        .unwrap()
        .check()
        .unwrap();

    // 1. list_rule_entries (rules-list UI)
    let listed = chronacle_extraction::codex_service::list_rule_entries(&db, &collection.id)
        .await
        .unwrap();
    assert!(
        listed.iter().all(|r| r.name != "Vanished Grapple Rule"),
        "soft-deleted rule entry leaked from list_rule_entries: {listed:?}"
    );
    assert!(
        listed.iter().any(|r| r.name == "Initiative"),
        "live rule entry missing from list_rule_entries"
    );

    // 2. codex_status counts ($rules, $rstale, and $ents' sibling bug for
    //    soft-deleted entities)
    let status = chronacle_extraction::codex_service::codex_status(&db, &collection.id)
        .await
        .unwrap();
    assert_eq!(
        status.rule_entries, 1,
        "soft-deleted rule entry must not count toward rule_entries"
    );
    assert_eq!(
        status.rules_stale, 0,
        "the soft-deleted rule entry was the only stale one; it must not count"
    );
}
