#![cfg(feature = "rocksdb")]

use surrealdb::engine::local::RocksDb;
use surrealdb::Surreal;

#[tokio::test]
async fn rocksdb_persistence_backend_round_trips_records() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("chronacle.db");

    let db = Surreal::new::<RocksDb>(&*path).await.expect("open RocksDB");
    db.use_ns("test").use_db("test").await.expect("select db");
    db.query("CREATE persistence_probe:one SET value = 'stored'")
        .await
        .expect("write record")
        .check()
        .expect("write succeeds");
    #[derive(serde::Deserialize)]
    struct Probe {
        value: String,
    }

    let record: Option<Probe> = db
        .select(("persistence_probe", "one"))
        .await
        .expect("read RocksDB record");

    assert_eq!(record.map(|row| row.value), Some("stored".to_owned()));
}
