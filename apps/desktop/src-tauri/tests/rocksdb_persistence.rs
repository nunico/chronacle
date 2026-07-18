#![cfg(feature = "rocksdb")]

use surrealdb::engine::local::RocksDb;
use surrealdb::Surreal;

#[tokio::test]
async fn rocksdb_records_survive_reopening_the_database() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("chronacle.db");

    let db = Surreal::new::<RocksDb>(&*path).await.expect("open RocksDB");
    db.use_ns("test").use_db("test").await.expect("select db");
    db.query("CREATE persistence_probe:one SET value = 'stored'")
        .await
        .expect("write record")
        .check()
        .expect("write succeeds");
    drop(db);

    // Dropping the last client closes its route channel; the embedded router
    // then shuts down background tasks and releases RocksDB asynchronously.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let reopened = Surreal::new::<RocksDb>(&*path)
        .await
        .expect("reopen RocksDB");
    reopened
        .use_ns("test")
        .use_db("test")
        .await
        .expect("select reopened db");
    #[derive(serde::Deserialize)]
    struct Probe {
        value: String,
    }

    let record: Option<Probe> = reopened
        .select(("persistence_probe", "one"))
        .await
        .expect("read RocksDB record");

    assert_eq!(record.map(|row| row.value), Some("stored".to_owned()));
}
