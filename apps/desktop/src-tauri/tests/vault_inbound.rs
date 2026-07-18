//! L2 (path-switch) + E3 (inbound reconcile) integration coverage.

use std::sync::Arc;

use chronacle_core::{VaultRecordStore, VaultRef, VaultStoreError};
use chronacle_domain::vault_record_store::SurrealVaultRecordStore;
use chronacle_lib::commands::vault_commands::configure_vault_path;
use chronacle_lib::services::settings_service;
use chronacle_providers::vault_store::LocalFsVaultStore;
use chronacle_vault::outbound::PendingWrites;
use chronacle_vault::reconcile::VaultSyncService;

async fn db() -> surrealdb::Surreal<surrealdb::engine::any::Any> {
    let db = surrealdb::engine::any::connect("mem://")
        .await
        .expect("mem");
    db.use_ns("t").use_db("t").await.unwrap();
    chronacle_db::run_migrations(&db).await.expect("migrations");
    db.query(
        "CREATE campaign:c1 SET name = 'SoV', system = '5e', \
             created_at = time::now(), updated_at = time::now(); \
         CREATE npc:n1 SET name = 'Seraphina', notes = 'N.', \
             created_at = time::now(), updated_at = time::now(); \
         RELATE campaign:c1->in_campaign->npc:n1;",
    )
    .await
    .expect("seed")
    .check()
    .expect("seed ok");
    db
}

fn svc_for(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    root: &std::path::Path,
) -> Arc<VaultSyncService> {
    Arc::new(VaultSyncService::new(
        Arc::new(LocalFsVaultStore::new(root.to_str().unwrap())),
        Arc::new(SurrealVaultRecordStore::new(db.clone())),
        Arc::new(PendingWrites::default()),
    ))
}

async fn subscribe_ready_for_dir(
    root: &std::path::Path,
    watched_dir: &std::path::Path,
) -> tokio::sync::mpsc::Receiver<chronacle_core::VaultEvent> {
    let watcher = chronacle_providers::vault_watcher::NotifyWatcher::with_debounce(
        root,
        std::time::Duration::from_millis(100),
    );
    let mut rx = chronacle_core::VaultWatcher::subscribe(&watcher).await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let probe = watched_dir.join(".chronacle-watch-ready.md");
    let probe_key = probe
        .strip_prefix(root)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");

    std::fs::write(&probe, "probe").unwrap();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let ev = tokio::time::timeout(remaining, rx.recv())
            .await
            .expect("watcher upsert probe within 5s")
            .expect("open channel");
        if ev == chronacle_core::VaultEvent::Upsert(probe_key.clone()) {
            break;
        }
    }

    std::fs::remove_file(&probe).unwrap();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let ev = tokio::time::timeout(remaining, rx.recv())
            .await
            .expect("watcher remove probe within 5s")
            .expect("open channel");
        if ev == chronacle_core::VaultEvent::Remove(probe_key.clone()) {
            break;
        }
    }

    rx
}

async fn wait_for_vault_event(
    rx: &mut tokio::sync::mpsc::Receiver<chronacle_core::VaultEvent>,
    expected: chronacle_core::VaultEvent,
    message: &str,
) {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let ev = tokio::time::timeout(remaining, rx.recv())
            .await
            .unwrap_or_else(|_| panic!("{message}"))
            .expect("open channel");
        if ev == expected {
            break;
        }
    }
}

#[tokio::test]
async fn switching_to_a_fresh_dir_after_clearing_bases_exports_cleanly() {
    let db = db().await;
    let dir_a = tempfile::TempDir::new().unwrap();
    let dir_b = tempfile::TempDir::new().unwrap();

    // First vault: export establishes a base.
    let a = svc_for(&db, dir_a.path());
    let r = a.reconcile().await.expect("reconcile a");
    assert_eq!(r.exported, 1);

    // Switch: fresh baseline, then reconcile against the empty dir B.
    let b = svc_for(&db, dir_b.path());
    b.clear_all_bases().await.expect("clear");
    let r = b.reconcile().await.expect("reconcile b");
    assert_eq!(
        r.exported, 1,
        "a fresh dir is a first export, not a deletion"
    );

    // The record's file exists in B; nothing was flagged deleted.
    let store = SurrealVaultRecordStore::new(db.clone());
    let vref = VaultRef {
        table: "npc".into(),
        id: "n1".into(),
    };
    assert!(store.get_synced_hash(&vref).await.expect("get").is_some());
}

/// A vault-path switch that fails must leave the OLD vault's merge bases in
/// force, not just its path.
///
/// `configure_vault_path` clears every base before reconciling the new root, so
/// a reconcile that then fails would otherwise leave the app pointed at the old
/// vault with no bases at all. That is not a harmless "it'll re-derive": the
/// base is the one piece of sync state nothing can reconstruct. With it gone,
/// the old vault's next reconcile sees `base = None` for every record, and any
/// file the GM edited outside the app reads as a fresh `Conflict` — a wave of
/// spurious sidecars caused by a switch that supposedly did nothing.
#[tokio::test]
async fn a_failing_switch_leaves_the_old_vaults_bases_intact() {
    let db = db().await;
    let dir_a = tempfile::TempDir::new().unwrap();
    let path_a = dir_a.path().to_str().unwrap().to_string();

    let a = svc_for(&db, dir_a.path());
    configure_vault_path(&db, &a, &path_a)
        .await
        .expect("initial configure exports and sets a base");

    let store = SurrealVaultRecordStore::new(db.clone());
    let before = store.list_synced().await.expect("bases before the switch");
    assert!(!before.is_empty(), "precondition: a base exists to lose");

    // A regular file as the root makes `read_dir` fail, so `VaultIndex::scan`
    // fails and `reconcile()` returns Err — a real failure, no mocks needed.
    let bad_root = dir_a.path().join("not-a-directory");
    std::fs::write(&bad_root, "nope").unwrap();
    let bad_path = bad_root.to_str().unwrap().to_string();

    let b = svc_for(&db, &bad_root);
    configure_vault_path(&db, &b, &bad_path)
        .await
        .expect_err("a broken root must fail the switch");

    let after = store.list_synced().await.expect("bases after the switch");
    assert_eq!(
        after, before,
        "a failed switch must restore the old vault's bases verbatim — \
         hash, key and conflict flag alike"
    );

    let persisted = settings_service::get_all(&db)
        .await
        .expect("settings")
        .into_iter()
        .find(|s| s.key == "vault_sync_path")
        .map(|s| s.value);
    assert_eq!(
        persisted.as_deref(),
        Some(path_a.as_str()),
        "the old path must still be in force"
    );
}

/// Drives the command-layer helper directly (no Tauri `State`), exercising
/// the exact ordering `set_vault_path` relies on: clear-if-changed → reconcile
/// → persist. This is what the two tests below prove pieces of.
#[tokio::test]
async fn resubmitting_the_same_path_preserves_the_base_and_exports_an_edit() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().to_str().unwrap().to_string();

    let svc = svc_for(&db, dir.path());
    configure_vault_path(&db, &svc, &path)
        .await
        .expect("first configure exports the initial record");

    let store = SurrealVaultRecordStore::new(db.clone());
    let vref = VaultRef {
        table: "npc".into(),
        id: "n1".into(),
    };
    let hash_1 = store
        .get_synced_hash(&vref)
        .await
        .expect("get")
        .expect("base set after first configure");

    // Edit the record in the DB without touching the vault file — this is
    // the "Sync now" scenario: same path, changed content.
    db.query("UPDATE npc:n1 SET notes = 'Revised.', updated_at = time::now()")
        .await
        .expect("edit")
        .check()
        .expect("edit ok");

    // Re-submitting the SAME path must NOT wipe the base. If it did, the
    // base would go to `None` while the on-disk file still holds the old
    // content, and since the DB now disagrees with both, `decide()` would
    // read that as a `Conflict` (deferred, no write, no base update) instead
    // of the plain `Export` it actually is — leaving `hash_1` stuck forever
    // instead of advancing.
    configure_vault_path(&db, &svc, &path)
        .await
        .expect("re-submitting the same path reconciles cleanly");

    let hash_2 = store
        .get_synced_hash(&vref)
        .await
        .expect("get")
        .expect("base must still be present, not wiped");
    assert_ne!(
        hash_2, hash_1,
        "the edit must be exported and the base advanced, not stuck at the pre-edit hash \
         (a base stuck at hash_1 means the base was wrongly cleared and the edit was \
         misread as a Conflict instead of an Export)"
    );
}

/// A `VaultStore` whose `list` always errors — `VaultIndex::scan` propagates
/// that with `?`, making `reconcile()` fail before it writes or touches any
/// base. Used to prove a failed reconcile never reaches the `upsert`.
fn failing_svc(db: &surrealdb::Surreal<surrealdb::engine::any::Any>) -> Arc<VaultSyncService> {
    let mut store = chronacle_core::MockVaultStore::new();
    store.expect_list().returning(|_| {
        Err(VaultStoreError::Io {
            kind: std::io::ErrorKind::Other,
            message: "simulated: cannot read vault root".into(),
        })
    });
    Arc::new(VaultSyncService::new(
        Arc::new(store),
        Arc::new(SurrealVaultRecordStore::new(db.clone())),
        Arc::new(PendingWrites::default()),
    ))
}

#[tokio::test]
async fn a_failing_reconcile_does_not_persist_the_new_vault_path() {
    let db = db().await;
    let dir_old = tempfile::TempDir::new().unwrap();
    let old_path = dir_old.path().to_str().unwrap().to_string();

    // Establish a working, persisted "old" configuration first.
    let old_svc = svc_for(&db, dir_old.path());
    configure_vault_path(&db, &old_svc, &old_path)
        .await
        .expect("initial configure succeeds");

    // Attempt to switch to a path whose reconcile always fails.
    let broken = failing_svc(&db);
    let err = configure_vault_path(&db, &broken, "/some/other/path")
        .await
        .expect_err("a failing reconcile must surface as an error");
    assert!(!err.is_empty());

    // The setting must still hold the OLD path — the failed switch did not
    // take effect. This fails if the `upsert` is moved back before the
    // `reconcile` call.
    let persisted = settings_service::get_all(&db)
        .await
        .expect("get_all")
        .into_iter()
        .find(|s| s.key == "vault_sync_path")
        .map(|s| s.value);
    assert_eq!(
        persisted.as_deref(),
        Some(old_path.as_str()),
        "a failed vault path switch must leave the previous path in force"
    );
}

#[tokio::test]
async fn gm_edit_round_trips_through_reconcile_into_the_db() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());
    svc.reconcile().await.expect("first export");

    // Find the exported file and append GM notes outside the fence.
    let path = dir
        .path()
        .join("campaigns/sov/entities/npc")
        .read_dir()
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{content}\n\nInbound edit from Obsidian.\n")).unwrap();

    let report = svc.reconcile().await.expect("inbound pass");
    assert_eq!(report.applied, 1);

    #[derive(serde::Deserialize)]
    struct Row {
        notes: Option<String>,
    }
    let mut resp = db
        .query("SELECT notes FROM npc:n1")
        .await
        .unwrap()
        .check()
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert!(rows[0]
        .notes
        .as_deref()
        .unwrap_or("")
        .contains("Inbound edit from Obsidian."));

    // Third pass: everything converged.
    let report = svc.reconcile().await.expect("settle");
    assert_eq!(report.unchanged, 1);
}

#[tokio::test]
async fn conflict_freezes_then_sidecar_deletion_resolves_to_the_file_version() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());
    svc.reconcile().await.expect("export");

    let path = dir
        .path()
        .join("campaigns/sov/entities/npc")
        .read_dir()
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();

    // Diverge BOTH sides: edit the file, and edit the DB notes.
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{content}\nVault-side edit.\n")).unwrap();
    db.query("UPDATE npc:n1 SET notes = 'App-side edit.'")
        .await
        .unwrap()
        .check()
        .unwrap();

    let report = svc.reconcile().await.expect("conflict pass");
    assert_eq!(report.conflicts, 1);
    let sidecar = path.with_file_name(format!(
        "{}.conflict.md",
        path.file_stem().unwrap().to_str().unwrap()
    ));
    assert!(sidecar.exists(), "DB version preserved in the sidecar");
    assert!(std::fs::read_to_string(&sidecar)
        .unwrap()
        .contains("App-side edit."));

    // Frozen: another pass changes nothing, file untouched.
    let report = svc.reconcile().await.expect("frozen pass");
    assert_eq!(report.conflicts, 1);
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains("Vault-side edit."));

    // GM resolves by deleting the sidecar.
    std::fs::remove_file(&sidecar).unwrap();
    let report = svc.reconcile().await.expect("resolution pass");
    assert_eq!(report.resolved, 1);

    #[derive(serde::Deserialize)]
    struct Row {
        notes: Option<String>,
    }
    let mut resp = db
        .query("SELECT notes FROM npc:n1")
        .await
        .unwrap()
        .check()
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert!(rows[0]
        .notes
        .as_deref()
        .unwrap()
        .contains("Vault-side edit."));
}

/// E5: soft-deleting an entity in the app must sweep its vault file on the
/// next reconcile — the same orphan-sweep path a hard delete already used.
#[tokio::test]
async fn soft_deleting_an_entity_sweeps_its_vault_file_on_reconcile() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());
    svc.reconcile().await.expect("first export");

    let entities_dir = dir.path().join("campaigns/sov/entities/npc");
    assert_eq!(entities_dir.read_dir().unwrap().count(), 1);

    chronacle_extraction::entity_service::soft_delete(
        &db,
        "n1",
        chronacle_extraction::entity_service::EntityKind::Npc,
    )
    .await
    .expect("soft delete");

    let report = svc.reconcile().await.expect("sweep pass");
    assert_eq!(report.soft_deleted, 0, "already soft-deleted, not newly so");
    assert_eq!(report.swept, 1, "orphaned sync row is swept");
    assert_eq!(
        entities_dir.read_dir().unwrap().count(),
        0,
        "the vault file must be removed"
    );
}

/// E5: `VaultSyncService::conflicts()` end-to-end — a genuine two-sided
/// divergence freezes the record, and `conflicts()` surfaces it with a
/// resolved display name and the sidecar's key.
#[tokio::test]
async fn conflicts_lists_a_frozen_record_end_to_end() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());
    svc.reconcile().await.expect("export");

    assert!(
        svc.conflicts().await.expect("conflicts").is_empty(),
        "nothing frozen yet"
    );

    let path = dir
        .path()
        .join("campaigns/sov/entities/npc")
        .read_dir()
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{content}\nVault-side edit.\n")).unwrap();
    db.query("UPDATE npc:n1 SET notes = 'App-side edit.'")
        .await
        .unwrap()
        .check()
        .unwrap();
    svc.reconcile().await.expect("conflict pass");

    let conflicts = svc.conflicts().await.expect("conflicts");
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].vref.table, "npc");
    assert_eq!(conflicts[0].vref.id, "n1");
    assert_eq!(conflicts[0].name, "Seraphina");
    assert!(conflicts[0].sidecar_key.ends_with(".conflict.md"));
}

// -- E6: NotifyWatcher end-to-end (watcher -> reconcile, no manual call) ---

#[tokio::test]
async fn a_vault_edit_flows_into_the_db_via_the_watcher() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());
    svc.reconcile().await.expect("export");

    let entity_dir = dir.path().join("campaigns/sov/entities/npc");
    let mut rx = subscribe_ready_for_dir(dir.path(), &entity_dir).await;
    let path = entity_dir
        .read_dir()
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{content}\nWatcher-driven edit.\n")).unwrap();

    // Mimic the consumer loop: event -> not our write -> reconcile.
    let ev = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("event")
        .expect("open");
    let chronacle_core::VaultEvent::Upsert(key) = ev else {
        panic!("expected upsert")
    };
    assert!(!svc.is_own_write(&key).await, "a GM edit is not our write");
    let report = svc.reconcile().await.expect("reconcile");
    assert_eq!(report.applied, 1);
}

#[tokio::test]
async fn our_own_export_is_recognised_by_the_guard() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());
    svc.reconcile().await.expect("export");
    // Every file reconcile just wrote must match the armed guard.
    let key = "campaigns/sov/entities/npc/seraphina.md";
    assert!(
        svc.is_own_write(key).await,
        "export must arm the guard (E1)"
    );
}

/// THE HAZARD (direction 1): reconcile's own cleanup deletes are compiler-
/// driven, not GM signals. When an evaporated conflict's sidecar cleanup
/// deletes `<key>.conflict.md`, the watcher must recognise that deletion as
/// its own and must NOT read it as the GM's "I merged, apply my file"
/// resolution signal.
#[tokio::test]
async fn our_own_sidecar_cleanup_is_not_mistaken_for_the_gms_resolution_signal() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());
    svc.reconcile().await.expect("export");

    let path = dir
        .path()
        .join("campaigns/sov/entities/npc")
        .read_dir()
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();

    let mut rx = subscribe_ready_for_dir(dir.path(), path.parent().unwrap()).await;

    // Diverge both sides to freeze a conflict; the DB render lands in the
    // sidecar.
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{content}\nVault-side edit.\n")).unwrap();
    db.query("UPDATE npc:n1 SET notes = 'App-side edit.'")
        .await
        .unwrap()
        .check()
        .unwrap();
    let report = svc.reconcile().await.expect("conflict pass");
    assert_eq!(report.conflicts, 1);

    let sidecar = path.with_file_name(format!(
        "{}.conflict.md",
        path.file_stem().unwrap().to_str().unwrap()
    ));
    let sidecar_key = "campaigns/sov/entities/npc/seraphina.conflict.md";
    let sidecar_content = std::fs::read_to_string(&sidecar).unwrap();

    // The conflict evaporates on its own: overwrite the primary file with
    // exactly the DB's current render (the sidecar's content) — no GM
    // resolution here, the file just happens to now match the DB.
    std::fs::write(&path, &sidecar_content).unwrap();
    let report = svc
        .reconcile()
        .await
        .expect("evaporated-conflict cleanup pass");
    assert_eq!(report.conflicts, 0, "the conflict evaporated");

    // Reconcile deletes the sidecar itself as part of that cleanup, arming
    // the delete guard first. Drain the watcher until it observes the
    // resulting Remove event (other Upsert events from the file edits above
    // may also arrive; they are not the point of this assertion).
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let ev = tokio::time::timeout(remaining, rx.recv())
            .await
            .expect("a remove event for the sidecar within 5s")
            .expect("open channel");
        if ev == chronacle_core::VaultEvent::Remove(sidecar_key.to_string()) {
            break;
        }
    }

    // Mimic the consumer loop's Remove handling: this must be recognised as
    // our own cleanup, not a GM resolution signal.
    assert!(
        svc.is_own_delete(sidecar_key),
        "our own evaporated-conflict sidecar cleanup must be recognised as our own delete"
    );
}

/// THE HAZARD (direction 2): a GM's own sidecar deletion IS the resolution
/// signal and must still reach `reconcile()` and take effect — the delete
/// guard must never blanket-suppress every `Remove` event, only the ones
/// Chronacle itself made.
#[tokio::test]
async fn a_gms_sidecar_deletion_is_not_masked_and_still_resolves_the_conflict() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());
    svc.reconcile().await.expect("export");

    let path = dir
        .path()
        .join("campaigns/sov/entities/npc")
        .read_dir()
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();

    let mut rx = subscribe_ready_for_dir(dir.path(), path.parent().unwrap()).await;

    // Diverge both sides to freeze a conflict (Chronacle writes the sidecar —
    // its own write, not under test here).
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{content}\nVault-side edit.\n")).unwrap();
    db.query("UPDATE npc:n1 SET notes = 'App-side edit.'")
        .await
        .unwrap()
        .check()
        .unwrap();
    svc.reconcile().await.expect("conflict pass");

    let sidecar = path.with_file_name(format!(
        "{}.conflict.md",
        path.file_stem().unwrap().to_str().unwrap()
    ));
    let sidecar_key = "campaigns/sov/entities/npc/seraphina.conflict.md";
    wait_for_vault_event(
        &mut rx,
        chronacle_core::VaultEvent::Upsert(sidecar_key.to_string()),
        "a sidecar upsert event within 5s",
    )
    .await;

    // The GM resolves by deleting the sidecar themselves.
    std::fs::remove_file(&sidecar).unwrap();

    wait_for_vault_event(
        &mut rx,
        chronacle_core::VaultEvent::Remove(sidecar_key.to_string()),
        "a remove event for the sidecar within 5s",
    )
    .await;

    // Mimic the consumer loop: not our delete -> relevant -> reconcile.
    assert!(
        !svc.is_own_delete(sidecar_key),
        "the GM's own sidecar deletion must not be masked as our own"
    );
    let report = svc.reconcile().await.expect("resolution pass");
    assert_eq!(
        report.resolved, 1,
        "the GM's resolution must still take effect"
    );

    #[derive(serde::Deserialize)]
    struct Row {
        notes: Option<String>,
    }
    let mut resp = db
        .query("SELECT notes FROM npc:n1")
        .await
        .unwrap()
        .check()
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert!(rows[0]
        .notes
        .as_deref()
        .unwrap()
        .contains("Vault-side edit."));
}

/// Finding 1, end-to-end: the exact masking sequence a non-consuming delete
/// guard would get wrong. Compiler deletes the sidecar (evaporated conflict)
/// -> the record re-diverges and the conflict recurs, so reconcile rewrites
/// the sidecar -> the GM deletes it themselves, a genuine resolution signal.
/// A stale, non-consuming guard from step 1 would still be "armed" and would
/// swallow the GM's `Remove` in step 3, freezing the conflict forever with
/// the GM believing they resolved it. The consuming guard (`take_delete`)
/// must let the second, independent deletion through.
#[tokio::test]
async fn a_gms_deletion_after_an_earlier_compiler_delete_of_the_same_key_still_resolves() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());
    svc.reconcile().await.expect("export");

    let path = dir
        .path()
        .join("campaigns/sov/entities/npc")
        .read_dir()
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let sidecar = path.with_file_name(format!(
        "{}.conflict.md",
        path.file_stem().unwrap().to_str().unwrap()
    ));
    let sidecar_key = "campaigns/sov/entities/npc/seraphina.conflict.md";

    // Round 1: diverge both sides -> freeze a conflict, sidecar written.
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{content}\nVault-side edit A.\n")).unwrap();
    db.query("UPDATE npc:n1 SET notes = 'App-side edit A.'")
        .await
        .unwrap()
        .check()
        .unwrap();
    let report = svc.reconcile().await.expect("first conflict pass");
    assert_eq!(report.conflicts, 1);
    assert!(sidecar.exists());

    // The conflict evaporates: the file is overwritten with exactly the
    // sidecar's (DB's) content, so reconcile deletes the sidecar itself
    // (compiler-driven), arming the delete guard for `sidecar_key`.
    let sidecar_content = std::fs::read_to_string(&sidecar).unwrap();
    std::fs::write(&path, &sidecar_content).unwrap();
    let report = svc.reconcile().await.expect("evaporated-conflict cleanup");
    assert_eq!(report.conflicts, 0, "the conflict evaporated");
    assert!(
        !sidecar.exists(),
        "reconcile's own cleanup deleted the sidecar"
    );

    // In production, the watcher observes this compiler-driven `Remove` and
    // calls `is_own_delete` once, consuming the guard — exactly what THE
    // MASKING BUG (Finding 1) needed a non-consuming guard to still exist
    // for at the GM's later, unrelated deletion of the same key.
    assert!(
        svc.is_own_delete(sidecar_key),
        "the compiler's own cleanup delete must be recognised as our own"
    );

    // Round 2: the record re-diverges -> the conflict recurs, and reconcile
    // rewrites the sidecar. This does NOT re-arm the delete guard — only a
    // delete arms it, and this pass performs a write, not a delete.
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{content}\nVault-side edit B.\n")).unwrap();
    db.query("UPDATE npc:n1 SET notes = 'App-side edit B.'")
        .await
        .unwrap()
        .check()
        .unwrap();
    let report = svc.reconcile().await.expect("second conflict pass");
    assert_eq!(report.conflicts, 1, "the conflict recurred");
    assert!(sidecar.exists(), "the sidecar was rewritten");

    // The GM deletes the sidecar themselves — a genuine resolution signal,
    // unrelated to round 1's compiler-driven delete of the same key.
    std::fs::remove_file(&sidecar).unwrap();

    // Mimic the watcher's Remove handling: with a consuming guard, this
    // deletion is NOT masked by the stale round-1 arm.
    assert!(
        !svc.is_own_delete(sidecar_key),
        "a genuine GM deletion must not be masked by an earlier, already-served \
         compiler-driven delete of the same key"
    );
    let report = svc.reconcile().await.expect("resolution pass");
    assert_eq!(
        report.resolved, 1,
        "the GM's second resolution must still take effect"
    );

    #[derive(serde::Deserialize)]
    struct Row2 {
        notes: Option<String>,
    }
    let mut resp = db
        .query("SELECT notes FROM npc:n1")
        .await
        .unwrap()
        .check()
        .unwrap();
    let rows: Vec<Row2> = resp.take(0).unwrap();
    assert!(rows[0]
        .notes
        .as_deref()
        .unwrap()
        .contains("Vault-side edit B."));
}
