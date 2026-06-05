use chronacle_lib::services::session_service::{
    create, delete, get_all, get_by_id, update, SessionError, SessionInput,
};
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

/// Create a campaign in the DB and return its raw ID.
async fn create_campaign(db: &Surreal<Db>) -> String {
    #[derive(serde::Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
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

fn make_input(number: i64, title: &str) -> SessionInput {
    SessionInput {
        session_number: number,
        title: title.to_string(),
        date_played: "2026-06-05".to_string(),
        notes: String::new(),
    }
}

// ── Test 1 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn create_session_returns_session_with_correct_fields() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;

    let session = create(&db, &campaign_id, make_input(1, "The Beginning"))
        .await
        .unwrap();

    assert_eq!(session.session_number, 1);
    assert_eq!(session.title, "The Beginning");
    assert_eq!(session.date_played, "2026-06-05");
    assert_eq!(session.campaign_id.as_deref(), Some(campaign_id.as_str()));
    assert!(!session.id.is_empty());
    assert!(session.created_at.is_some());
    assert!(session.updated_at.is_some());
}

// ── Test 2 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn create_session_requires_nonempty_title() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;

    let err = create(
        &db,
        &campaign_id,
        SessionInput {
            session_number: 1,
            title: "   ".to_string(),
            date_played: "2026-06-05".to_string(),
            notes: String::new(),
        },
    )
    .await
    .unwrap_err();

    assert!(
        matches!(&err, SessionError::Validation { field, .. } if field == "title"),
        "expected Validation(title), got {err:?}"
    );
}

// ── Test 3 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn create_session_requires_positive_session_number() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;

    for bad_number in [0_i64, -1, -99] {
        let err = create(
            &db,
            &campaign_id,
            SessionInput {
                session_number: bad_number,
                title: "Valid Title".to_string(),
                date_played: "2026-06-05".to_string(),
                notes: String::new(),
            },
        )
        .await
        .unwrap_err();

        assert!(
            matches!(&err, SessionError::Validation { field, .. } if field == "session_number"),
            "expected Validation(session_number) for number={bad_number}, got {err:?}"
        );
    }
}

// ── Test 4 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_all_sessions_ordered_by_session_number() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;

    // Insert out of order
    create(&db, &campaign_id, make_input(3, "Third"))
        .await
        .unwrap();
    create(&db, &campaign_id, make_input(1, "First"))
        .await
        .unwrap();
    create(&db, &campaign_id, make_input(2, "Second"))
        .await
        .unwrap();

    let sessions = get_all(&db, &campaign_id).await.unwrap();

    assert_eq!(sessions.len(), 3);
    assert_eq!(sessions[0].session_number, 1);
    assert_eq!(sessions[1].session_number, 2);
    assert_eq!(sessions[2].session_number, 3);
    assert_eq!(sessions[0].title, "First");
}

// ── Test 5 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_session_by_id_returns_correct_session() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;

    let created = create(&db, &campaign_id, make_input(1, "Session One"))
        .await
        .unwrap();

    let fetched = get_by_id(&db, &created.id).await.unwrap();

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.title, "Session One");
    assert_eq!(fetched.session_number, 1);
}

// ── Test 6 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_session_by_id_not_found_returns_error() {
    let db = setup_db().await;

    let err = get_by_id(&db, "nonexistentid").await.unwrap_err();

    assert!(
        matches!(&err, SessionError::NotFound { id } if id == "nonexistentid"),
        "expected NotFound, got {err:?}"
    );
}

// ── Test 7 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn update_session_changes_fields_and_updates_timestamp() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;

    let created = create(&db, &campaign_id, make_input(1, "Old Title"))
        .await
        .unwrap();

    // Grab original timestamp for comparison — it's an ISO-8601 string.
    let original_updated_at = created.updated_at.clone();

    // Small sleep to ensure updated_at will differ.
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let updated = update(
        &db,
        &created.id,
        &campaign_id,
        SessionInput {
            session_number: 1,
            title: "New Title".to_string(),
            date_played: "2026-07-01".to_string(),
            notes: "Updated notes".to_string(),
        },
    )
    .await
    .unwrap();

    assert_eq!(updated.id, created.id);
    assert_eq!(updated.title, "New Title");
    assert_eq!(updated.date_played, "2026-07-01");
    assert_eq!(updated.notes, "Updated notes");
    // updated_at should have advanced
    assert_ne!(
        updated.updated_at, original_updated_at,
        "updated_at must change after update"
    );
}

// ── Test 8 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_session_removes_record() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;

    let session = create(&db, &campaign_id, make_input(1, "To Be Deleted"))
        .await
        .unwrap();

    delete(&db, &session.id).await.unwrap();

    let err = get_by_id(&db, &session.id).await.unwrap_err();
    assert!(
        matches!(&err, SessionError::NotFound { .. }),
        "expected NotFound after delete, got {err:?}"
    );
}
