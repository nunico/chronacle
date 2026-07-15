//! Inbound apply must work on entities that predate the codex migration.
//!
//! Found by running the real app against a real campaign: every inbound apply
//! failed with
//!
//! ```text
//! Found NONE for field `codex_sources` ... but expected a array<object>
//! ```
//!
//! `DEFINE FIELD ... TYPE array<object> DEFAULT []` is a **write-time** default:
//! it does not backfill rows written before the field was defined, so those rows
//! hold no value at all. SurrealDB re-validates every field of a SCHEMAFULL
//! record on UPDATE, so a single unset non-optional field makes *any* later
//! write to that record fail — including the vault's inbound apply.
//!
//! This is the same class as the `vault_deleted != true` rule (a `DEFAULT` never
//! backfills); here it breaks writes rather than reads. Any GM with a campaign
//! older than the codex migration would find vault edits silently not applying.

use std::sync::Arc;

use chronacle_core::{GmParts, VaultRecordStore, VaultRef};
use chronacle_domain::vault_record_store::SurrealVaultRecordStore;

/// Seed an entity BEFORE migrations run, so it genuinely carries no
/// `codex_sources` value — exactly like a record created by an older build.
/// `UPDATE ... SET codex_sources = NONE` cannot reproduce this: the schema
/// rejects it, and the field would stay `[]`.
async fn db_with_a_pre_migration_npc() -> surrealdb::Surreal<surrealdb::engine::any::Any> {
    let db = surrealdb::engine::any::connect("mem://")
        .await
        .expect("mem");
    db.use_ns("t").use_db("t").await.unwrap();

    db.query(
        "CREATE campaign:c1 SET name = 'SoV', system = '5e', \
             created_at = time::now(), updated_at = time::now(); \
         CREATE npc:old SET name = 'Johar', summary = 'S.', notes = 'N.', \
             created_at = time::now(), updated_at = time::now(); \
         RELATE campaign:c1->in_campaign->npc:old;",
    )
    .await
    .expect("seed")
    .check()
    .expect("seed ok");

    // Precondition, asserted BEFORE migrations run: the row genuinely carries
    // no `codex_sources` value at all. Without this the test would still pass
    // against the broken schema, because a row created *after* the migration
    // picks up the write-time DEFAULT and never reproduces the bug.
    #[derive(serde::Deserialize)]
    struct Row {
        codex_sources: Option<Vec<serde_json::Value>>,
    }
    let mut r = db
        .query("SELECT codex_sources FROM npc:old")
        .await
        .expect("select")
        .check()
        .expect("select ok");
    let rows: Vec<Row> = r.take(0).expect("take");
    assert!(
        rows[0].codex_sources.is_none(),
        "precondition: the pre-migration row must carry NO codex_sources value"
    );

    // `run_migrations` defines the fields AND backfills the rows that predate
    // them — that backfill is what this test exists to protect.
    chronacle_db::run_migrations(&db).await.expect("migrations");
    db
}

#[tokio::test]
async fn inbound_apply_succeeds_on_a_record_created_before_the_codex_migration() {
    let db = db_with_a_pre_migration_npc().await;

    let store: Arc<dyn VaultRecordStore> = Arc::new(SurrealVaultRecordStore::new(db.clone()));
    let vref = VaultRef {
        table: "npc".into(),
        id: "old".into(),
    };

    // The GM edited this NPC's notes in their vault.
    store
        .apply_gm_parts(
            &vref,
            &GmParts {
                summary: Some("S.".into()),
                notes: Some("He owes the Syndicate. (Edited in Obsidian.)".into()),
                aliases: vec![],
            },
        )
        .await
        .expect("a vault edit must apply to an entity older than the codex migration");

    #[derive(serde::Deserialize)]
    struct Notes {
        notes: Option<String>,
    }
    let mut r = db
        .query("SELECT notes FROM npc:old")
        .await
        .expect("select")
        .check()
        .expect("select ok");
    let rows: Vec<Notes> = r.take(0).expect("take");
    assert_eq!(
        rows[0].notes.as_deref(),
        Some("He owes the Syndicate. (Edited in Obsidian.)"),
        "the GM's vault edit must reach the DB"
    );
}
