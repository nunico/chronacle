use serde::Deserialize;

use super::query::list_all_source_ids;

#[tokio::test]
async fn list_all_source_ids_returns_all_ids() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    crate::schema::run_migrations(&db).await.unwrap();
    db.query(
        "CREATE collection SET id='col1', name='Test', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    db.query(
        "CREATE source SET id='s1', filename='a.pdf', display_name='a', \
         source_type='rules', page_count=0, indexed_at=time::now(), \
         index_status='done', embed_model='nomic-embed-text-v1.5', \
         collection=type::thing('collection','col1')",
    )
    .await
    .unwrap();
    db.query(
        "CREATE source SET id='s2', filename='b.pdf', display_name='b', \
         source_type='rules', page_count=0, indexed_at=time::now(), \
         index_status='done', embed_model='nomic-embed-text-v1.5', \
         collection=type::thing('collection','col1')",
    )
    .await
    .unwrap();

    let ids = list_all_source_ids(&db).await.unwrap();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"s1".to_string()));
    assert!(ids.contains(&"s2".to_string()));
}

/// Regression test for the backtick-wrapped-ID bug. UUIDs contain hyphens,
/// which trigger SurrealDB's `EscapeRidKey` when `Id::to_string()` is used.
/// `list_all_source_ids` MUST return raw IDs so they can be passed back
/// through `type::thing('source', $id)` without producing a mangled record
/// reference. See commit e099a79 for the prior occurrence in delete_source.
#[tokio::test]
async fn list_all_source_ids_does_not_wrap_uuids_in_backticks() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    crate::schema::run_migrations(&db).await.unwrap();

    let uuid = "d5a80195-3968-44cb-8b46-270830df952f";
    db.query(
        "CREATE collection SET id='col1', name='Test', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    db.query(format!(
        "CREATE source SET id='{uuid}', filename='a.pdf', display_name='a', \
         source_type='rules', page_count=0, indexed_at=time::now(), \
         index_status='done', embed_model='nomic-embed-text-v1.5', \
         collection=type::thing('collection','col1')"
    ))
    .await
    .unwrap();

    let ids = list_all_source_ids(&db).await.unwrap();
    assert_eq!(ids.len(), 1);
    let id = &ids[0];
    assert!(
        !id.contains('`'),
        "ID must not be wrapped in backticks: got {id:?}"
    );
    assert_eq!(id, uuid);

    // Round-trip check: the returned ID must work with type::thing.
    // If the bug recurs, this query returns no rows.
    let mut resp = db
        .query("SELECT id FROM source WHERE id = type::thing('source', $id)")
        .bind(("id", id.clone()))
        .await
        .unwrap();
    #[derive(Deserialize)]
    struct Found {
        #[allow(dead_code)] // test probe — we only care about count
        id: surrealdb::sql::Thing,
    }
    let found: Vec<Found> = resp.take(0).unwrap();
    assert_eq!(
        found.len(),
        1,
        "round-trip lookup with raw ID must find the source"
    );
}
