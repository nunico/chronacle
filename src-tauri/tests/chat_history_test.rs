use chronacle_lib::services::agent_service::{persist_assistant_message, persist_message};
use chronacle_lib::services::campaign_service;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;

async fn setup_db() -> Surreal<Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_lib::schema::run_migrations(&db).await.unwrap();
    db
}

/// Mirror the exact SQL `commands::get_chat_history` issues.
async fn fetch_history(db: &Surreal<Db>, campaign_id: Option<&str>) -> Vec<(String, String)> {
    #[derive(serde::Deserialize)]
    struct Row {
        role: String,
        content: String,
    }

    let sql = match campaign_id {
        Some(cid) => {
            let safe_id = cid.replace('`', "``");
            format!(
                "SELECT role, content, created_at FROM message \
                 WHERE campaign = campaign:`{safe_id}` ORDER BY created_at ASC"
            )
        }
        None => {
            "SELECT role, content, created_at FROM message ORDER BY created_at ASC".to_string()
        }
    };

    let mut resp = db.query(sql).await.unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    rows.into_iter().map(|r| (r.role, r.content)).collect()
}

// ── Test 1 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn persisted_user_message_is_in_history() {
    let db = setup_db().await;

    persist_message(&db, "user", "Hello...", None)
        .await
        .unwrap();

    let history = fetch_history(&db, None).await;

    assert_eq!(history.len(), 1, "expected exactly 1 message");
    assert_eq!(history[0].0, "user", "role must be 'user'");
    assert_eq!(history[0].1, "Hello...", "content must match");
}

// ── Test 2 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn global_message_excluded_from_campaign_history() {
    let db = setup_db().await;

    // Persist a global message (no campaign).
    persist_message(&db, "user", "global message", None)
        .await
        .unwrap();

    // Create a campaign and persist a campaign-scoped message.
    let campaign = campaign_service::create(&db, "My Campaign", "D&D 5e")
        .await
        .unwrap();
    persist_message(&db, "user", "campaign message", Some(&campaign.id))
        .await
        .unwrap();

    let history = fetch_history(&db, Some(&campaign.id)).await;

    assert_eq!(
        history.len(),
        1,
        "only the campaign-scoped message should appear"
    );
    assert_eq!(history[0].1, "campaign message");
}

// ── Test 3 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn campaign_messages_in_global_query() {
    let db = setup_db().await;

    // Persist a global message.
    persist_message(&db, "user", "global message", None)
        .await
        .unwrap();

    // Create a campaign and persist a campaign-scoped message.
    let campaign = campaign_service::create(&db, "My Campaign", "D&D 5e")
        .await
        .unwrap();
    persist_message(&db, "user", "campaign message", Some(&campaign.id))
        .await
        .unwrap();

    // The None query has no WHERE filter — both messages should be returned.
    let history = fetch_history(&db, None).await;

    assert_eq!(history.len(), 2, "both messages should appear in global query");
}

// ── Test 4 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn persist_assistant_message_stores_content() {
    let db = setup_db().await;

    persist_assistant_message(&db, "A paladin is a holy warrior.", None)
        .await
        .unwrap();

    let history = fetch_history(&db, None).await;

    assert_eq!(history.len(), 1, "expected exactly 1 message");
    assert_eq!(history[0].0, "assistant", "role must be 'assistant'");
    assert!(
        history[0].1.contains("paladin"),
        "content must contain 'paladin', got: {:?}",
        history[0].1
    );
}

// ── Test 5 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn messages_ordered_by_creation_time() {
    let db = setup_db().await;

    persist_message(&db, "user", "first", None).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    persist_assistant_message(&db, "second", None).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    persist_message(&db, "user", "third", None).await.unwrap();

    let history = fetch_history(&db, None).await;

    assert_eq!(history.len(), 3, "expected 3 messages");
    assert_eq!(history[0].1, "first", "first message out of order");
    assert_eq!(history[1].1, "second", "second message out of order");
    assert_eq!(history[2].1, "third", "third message out of order");
}
