//! SurrealQL implementation of the `VaultRecordStore` port.
//!
//! Lives in `chronacle-domain` (not in the engine) so `chronacle-vault` stays
//! free of SurrealDB. Delegates entity semantics to `chronacle-extraction`'s
//! `entity_service` where writes are needed — `chronacle-domain` already
//! depends on `chronacle-extraction`, so no cycle arises.

use std::collections::HashMap;

use async_trait::async_trait;
use chronacle_core::{
    EntityRecord, GmParts, RuleEntryRecord, RulePageRef, SessionRecord, SyncedRow, VaultRecord,
    VaultRecordError, VaultRecordStore, VaultRef, VaultScope,
};
use serde::Deserialize;
use surrealdb::{engine::any::Any, sql::Thing, Surreal};

/// The eight per-type entity tables. There is no `entity` table.
const ENTITY_TABLES: [&str; 8] = [
    "npc",
    "location",
    "faction",
    "creature",
    "item",
    "event",
    "player_character",
    "misc",
];

/// `VaultRecordStore` backed by the embedded SurrealDB.
pub struct SurrealVaultRecordStore {
    db: Surreal<Any>,
}

impl SurrealVaultRecordStore {
    /// Wrap a live database handle.
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }
}

// ── Row shapes ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
struct EntityRow {
    id: Thing,
    name: String,
    summary: Option<String>,
    notes: Option<String>,
    codex_article: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    created_at: String,
    updated_at: String,
    campaign_id: Option<Thing>,
    collection_id: Option<Thing>,
}

#[derive(Debug, Clone, Deserialize)]
struct SessionRow {
    id: Thing,
    session_number: i64,
    title: String,
    date_played: String,
    notes: String,
    campaign: Thing,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RuleEntryRow {
    id: Thing,
    name: String,
    category: String,
    body: String,
    notes: Option<String>,
    #[serde(default)]
    page_refs: Vec<RulePageRef>,
    #[serde(default)]
    aliases: Vec<String>,
    collection: Thing,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct NamedRecord {
    id: Thing,
    name: String,
}

// ── VaultRecordStore impl ────────────────────────────────────────────────────

#[async_trait]
impl VaultRecordStore for SurrealVaultRecordStore {
    async fn list_all(&self) -> Result<Vec<VaultRecord>, VaultRecordError> {
        let mut records = Vec::new();

        // Names for scope resolution, fetched once and joined in Rust.
        let campaign_names = self.campaign_names().await?;
        let collection_names = self.collection_names().await?;

        for table in ENTITY_TABLES {
            let mut response = self
                .db
                .query(
                    "SELECT id, name, summary, notes, codex_article, aliases, created_at, updated_at, \
                         (SELECT VALUE in FROM in_campaign  WHERE out = $parent.id)[0]  AS campaign_id, \
                         (SELECT VALUE in FROM in_collection WHERE out = $parent.id)[0] AS collection_id \
                     FROM type::table($table) \
                     WHERE vault_deleted != true",
                )
                .bind(("table", table.to_owned()))
                .await
                .map_err(backend_err)?
                .check()
                .map_err(backend_err)?;

            let rows: Vec<EntityRow> = response.take(0).map_err(backend_err)?;
            for row in rows {
                let scope = if let Some(campaign_id) = row.campaign_id {
                    let raw = campaign_id.id.to_raw();
                    let name = campaign_names.get(&raw).cloned().unwrap_or_default();
                    VaultScope::Campaign { id: raw, name }
                } else if let Some(collection_id) = row.collection_id {
                    let raw = collection_id.id.to_raw();
                    let name = collection_names.get(&raw).cloned().unwrap_or_default();
                    VaultScope::Collection { id: raw, name }
                } else {
                    // No scope edge — unreachable in the UI. Skip.
                    continue;
                };

                records.push(VaultRecord::Entity(EntityRecord {
                    vref: VaultRef {
                        table: row.id.tb.clone(),
                        id: row.id.id.to_raw(),
                    },
                    name: row.name,
                    summary: row.summary,
                    notes: row.notes,
                    codex_article: row.codex_article,
                    aliases: row.aliases,
                    scope,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                }));
            }
        }

        // Sessions.
        let mut response = self
            .db
            .query(
                "SELECT id, session_number, title, date_played, notes, campaign, \
                     created_at, updated_at \
                 FROM session \
                 WHERE vault_deleted != true",
            )
            .await
            .map_err(backend_err)?
            .check()
            .map_err(backend_err)?;
        let rows: Vec<SessionRow> = response.take(0).map_err(backend_err)?;
        for row in rows {
            let raw = row.campaign.id.to_raw();
            let name = campaign_names.get(&raw).cloned().unwrap_or_default();
            records.push(VaultRecord::Session(SessionRecord {
                vref: VaultRef {
                    table: row.id.tb.clone(),
                    id: row.id.id.to_raw(),
                },
                session_number: row.session_number,
                title: row.title,
                date_played: row.date_played,
                notes: row.notes,
                campaign: VaultScope::Campaign { id: raw, name },
                created_at: row.created_at,
                updated_at: row.updated_at,
            }));
        }

        // Rule entries.
        let mut response = self
            .db
            .query(
                "SELECT id, name, category, body, notes, page_refs, aliases, collection, \
                     created_at, updated_at \
                 FROM rule_entry \
                 WHERE vault_deleted != true",
            )
            .await
            .map_err(backend_err)?
            .check()
            .map_err(backend_err)?;
        let rows: Vec<RuleEntryRow> = response.take(0).map_err(backend_err)?;
        for row in rows {
            let raw = row.collection.id.to_raw();
            let name = collection_names.get(&raw).cloned().unwrap_or_default();
            records.push(VaultRecord::RuleEntry(RuleEntryRecord {
                vref: VaultRef {
                    table: row.id.tb.clone(),
                    id: row.id.id.to_raw(),
                },
                name: row.name,
                category: row.category,
                body: row.body,
                notes: row.notes,
                page_refs: row.page_refs,
                aliases: row.aliases,
                collection: VaultScope::Collection { id: raw, name },
                created_at: row.created_at,
                updated_at: row.updated_at,
            }));
        }

        Ok(records)
    }

    async fn load(&self, vref: &VaultRef) -> Result<Option<VaultRecord>, VaultRecordError> {
        // No single-record fast path yet; filter list_all. Record counts are
        // small enough (desktop, single-user) that this is not a concern.
        let all = self.list_all().await?;
        Ok(all.into_iter().find(|r| record_vref(r) == vref))
    }

    async fn get_synced_hash(&self, vref: &VaultRef) -> Result<Option<u64>, VaultRecordError> {
        #[derive(Debug, Deserialize)]
        struct Row {
            synced_hash: String,
        }

        let mut response = self
            .db
            .query("SELECT synced_hash FROM vault_sync_state WHERE record = $record")
            .bind(("record", vref.to_thing()))
            .await
            .map_err(backend_err)?
            .check()
            .map_err(backend_err)?;
        let rows: Vec<Row> = response.take(0).map_err(backend_err)?;
        Ok(rows
            .into_iter()
            .next()
            .and_then(|r| r.synced_hash.parse::<u64>().ok()))
    }

    async fn set_synced_hash(
        &self,
        vref: &VaultRef,
        key: &str,
        hash: u64,
    ) -> Result<(), VaultRecordError> {
        let record = vref.to_thing();
        self.db
            .query(
                "UPSERT type::thing('vault_sync_state', $record) \
                 SET record = $record, key = $key, synced_hash = $hash, synced_at = time::now()",
            )
            .bind(("record", record))
            .bind(("key", key.to_owned()))
            .bind(("hash", hash.to_string()))
            .await
            .map_err(backend_err)?
            .check()
            .map_err(backend_err)?;
        Ok(())
    }

    async fn clear_synced_hash(&self, vref: &VaultRef) -> Result<(), VaultRecordError> {
        self.db
            .query("DELETE vault_sync_state WHERE record = $record")
            .bind(("record", vref.to_thing()))
            .await
            .map_err(backend_err)?
            .check()
            .map_err(backend_err)?;
        Ok(())
    }

    async fn clear_all_synced(&self) -> Result<(), VaultRecordError> {
        self.db
            .query("DELETE vault_sync_state")
            .await
            .map_err(backend_err)?
            .check()
            .map_err(backend_err)?;
        Ok(())
    }

    async fn restore_synced(&self, rows: &[SyncedRow]) -> Result<(), VaultRecordError> {
        for row in rows {
            // `synced_hash` is a string on the wire and `''` reads back as no
            // base (`None`), so a conflict-only row round-trips unchanged.
            self.db
                .query(
                    "UPSERT type::thing('vault_sync_state', $record) \
                     SET record = $record, key = $key, synced_hash = $hash, \
                         conflict = $conflict, synced_at = time::now()",
                )
                .bind(("record", row.vref.to_thing()))
                .bind(("key", row.key.clone()))
                .bind((
                    "hash",
                    row.synced_hash.map(|h| h.to_string()).unwrap_or_default(),
                ))
                .bind(("conflict", row.conflict))
                .await
                .map_err(backend_err)?
                .check()
                .map_err(backend_err)?;
        }
        Ok(())
    }

    async fn list_synced(&self) -> Result<Vec<SyncedRow>, VaultRecordError> {
        #[derive(Debug, Deserialize)]
        struct Row {
            record: String,
            key: String,
            synced_hash: String,
            #[serde(default)]
            conflict: bool,
        }
        let mut response = self
            .db
            .query("SELECT record, key, synced_hash, conflict FROM vault_sync_state")
            .await
            .map_err(backend_err)?
            .check()
            .map_err(backend_err)?;
        let rows: Vec<Row> = response.take(0).map_err(backend_err)?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                let vref = VaultRef::parse(&r.record)?;
                Some(SyncedRow {
                    vref,
                    key: r.key,
                    synced_hash: r.synced_hash.parse::<u64>().ok(),
                    conflict: r.conflict,
                })
            })
            .collect())
    }

    async fn apply_gm_parts(
        &self,
        vref: &VaultRef,
        parts: &GmParts,
    ) -> Result<(), VaultRecordError> {
        // `Option::None` binds as NONE, which SCHEMAFULL `… | NULL` rejects —
        // bind explicit NULL (same convention as entity_service::update).
        fn opt(o: &Option<String>) -> surrealdb::sql::Value {
            o.clone().map_or(surrealdb::sql::Value::Null, Into::into)
        }
        match vref.table.as_str() {
            "session" => {
                self.db
                    .query(
                        "UPDATE type::thing('session', $id) SET \
                             notes = $notes, updated_at = time::now()",
                    )
                    .bind(("id", vref.id.clone()))
                    .bind(("notes", parts.notes.clone().unwrap_or_default()))
                    .await
                    .map_err(backend_err)?
                    .check()
                    .map_err(backend_err)?;
            }
            "rule_entry" => {
                self.db
                    .query(
                        "UPDATE type::thing('rule_entry', $id) SET \
                             notes = $notes, aliases = $aliases, updated_at = time::now()",
                    )
                    .bind(("id", vref.id.clone()))
                    .bind(("notes", opt(&parts.notes)))
                    .bind(("aliases", parts.aliases.clone()))
                    .await
                    .map_err(backend_err)?
                    .check()
                    .map_err(backend_err)?;
            }
            table if ENTITY_TABLES.contains(&table) => {
                self.db
                    .query(
                        "UPDATE type::thing($table, $id) SET \
                             summary = $summary, notes = $notes, aliases = $aliases, \
                             codex_stale = true, updated_at = time::now()",
                    )
                    .bind(("table", table.to_owned()))
                    .bind(("id", vref.id.clone()))
                    .bind(("summary", opt(&parts.summary)))
                    .bind(("notes", opt(&parts.notes)))
                    .bind(("aliases", parts.aliases.clone()))
                    .await
                    .map_err(backend_err)?
                    .check()
                    .map_err(backend_err)?;
                // Keep wikilinks (relates_to edges) consistent with the new notes,
                // exactly as an in-app edit would (entity_service::update does this).
                if let Some(notes) = &parts.notes {
                    if let Ok(Some(VaultRecord::Entity(e))) = self.load(vref).await {
                        use chronacle_extraction::wikilink::{
                            parse_and_sync_wikilinks, WikilinkScope,
                        };
                        let scope = match &e.scope {
                            VaultScope::Campaign { id, .. } => {
                                WikilinkScope::Campaign { campaign_id: id }
                            }
                            VaultScope::Collection { id, .. } => {
                                WikilinkScope::Collection { collection_id: id }
                            }
                        };
                        if let Err(e) =
                            parse_and_sync_wikilinks(&self.db, table, &vref.id, notes, scope).await
                        {
                            // Non-fatal: a failed wikilink resync must not fail the
                            // inbound apply, but it must not vanish silently either.
                            eprintln!(
                                "apply_gm_parts: wikilink resync failed for {}:{}: {e}",
                                table, vref.id
                            );
                        }
                    }
                }
            }
            other => {
                return Err(VaultRecordError::Backend(format!(
                    "apply_gm_parts: unsupported table {other}"
                )))
            }
        }
        Ok(())
    }

    async fn soft_delete(&self, vref: &VaultRef) -> Result<(), VaultRecordError> {
        self.db
            .query("UPDATE type::thing($table, $id) SET vault_deleted = true")
            .bind(("table", vref.table.clone()))
            .bind(("id", vref.id.clone()))
            .await
            .map_err(backend_err)?
            .check()
            .map_err(backend_err)?;
        Ok(())
    }

    async fn set_conflict(
        &self,
        vref: &VaultRef,
        key: &str,
        in_conflict: bool,
    ) -> Result<(), VaultRecordError> {
        let record = vref.to_thing();
        self.db
            .query(
                "UPSERT type::thing('vault_sync_state', $record) \
                 SET record = $record, key = $key, conflict = $flag",
            )
            .bind(("record", record))
            .bind(("key", key.to_owned()))
            .bind(("flag", in_conflict))
            .await
            .map_err(backend_err)?
            .check()
            .map_err(backend_err)?;
        Ok(())
    }
}

impl SurrealVaultRecordStore {
    async fn campaign_names(&self) -> Result<HashMap<String, String>, VaultRecordError> {
        let mut response = self
            .db
            .query("SELECT id, name FROM campaign")
            .await
            .map_err(backend_err)?
            .check()
            .map_err(backend_err)?;
        let rows: Vec<NamedRecord> = response.take(0).map_err(backend_err)?;
        Ok(rows
            .into_iter()
            .map(|r| (r.id.id.to_raw(), r.name))
            .collect())
    }

    async fn collection_names(&self) -> Result<HashMap<String, String>, VaultRecordError> {
        let mut response = self
            .db
            .query("SELECT id, name FROM collection")
            .await
            .map_err(backend_err)?
            .check()
            .map_err(backend_err)?;
        let rows: Vec<NamedRecord> = response.take(0).map_err(backend_err)?;
        Ok(rows
            .into_iter()
            .map(|r| (r.id.id.to_raw(), r.name))
            .collect())
    }
}

fn record_vref(record: &VaultRecord) -> &VaultRef {
    match record {
        VaultRecord::Entity(e) => &e.vref,
        VaultRecord::Session(s) => &s.vref,
        VaultRecord::RuleEntry(r) => &r.vref,
    }
}

fn backend_err(e: impl std::fmt::Display) -> VaultRecordError {
    VaultRecordError::Backend(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronacle_core::{VaultRecord, VaultRecordStore, VaultRef, VaultScope};

    async fn db() -> surrealdb::Surreal<surrealdb::engine::any::Any> {
        let db = surrealdb::engine::any::connect("mem://")
            .await
            .expect("mem db");
        db.use_ns("test").use_db("test").await.unwrap();
        chronacle_db::run_migrations(&db).await.expect("migrations");
        db
    }

    async fn seed_campaign_npc(db: &surrealdb::Surreal<surrealdb::engine::any::Any>) {
        db.query(
            "CREATE campaign:c1 SET name = 'Shadows of Valdris', system = '5e', \
                 created_at = time::now(), updated_at = time::now(); \
             CREATE npc:n1 SET name = 'Seraphina Aldric', summary = 'Archivist', \
                 notes = 'GM notes', codex_article = 'Compiled.', \
                 created_at = time::now(), updated_at = time::now(); \
             RELATE campaign:c1->in_campaign->npc:n1;",
        )
        .await
        .expect("seed")
        .check()
        .expect("seed response");
    }

    #[tokio::test]
    async fn list_all_returns_entities_with_their_campaign_scope() {
        let db = db().await;
        seed_campaign_npc(&db).await;
        let store = SurrealVaultRecordStore::new(db);

        let records = store.list_all().await.expect("list_all");
        let entity = records
            .iter()
            .find_map(|r| match r {
                VaultRecord::Entity(e) => Some(e),
                _ => None,
            })
            .expect("one npc");

        assert_eq!(
            entity.vref,
            VaultRef {
                table: "npc".into(),
                id: "n1".into()
            }
        );
        assert_eq!(entity.name, "Seraphina Aldric");
        assert_eq!(entity.codex_article.as_deref(), Some("Compiled."));
        assert!(
            matches!(&entity.scope, VaultScope::Campaign { name, .. } if name == "Shadows of Valdris")
        );
    }

    #[tokio::test]
    async fn list_all_excludes_soft_deleted_records() {
        let db = db().await;
        seed_campaign_npc(&db).await;
        db.query("UPDATE npc:n1 SET vault_deleted = true")
            .await
            .expect("soft delete");
        let store = SurrealVaultRecordStore::new(db);

        let records = store.list_all().await.expect("list_all");
        assert!(records.is_empty(), "soft-deleted records must not sync");
    }

    /// The `!= true` rule: a row created before the migration has no
    /// `vault_deleted` value, and a `= false` filter would silently drop it.
    ///
    /// A genuinely unset field only arises on a *pre-migration* row: one created
    /// before `003_vault_sync.surql` defines `vault_deleted`, so the field's
    /// `DEFAULT false` never applies (DEFAULT is a write-time default, not a
    /// backfill). `UPDATE ... SET vault_deleted = NONE` cannot reproduce this —
    /// the `TYPE bool` constraint rejects NONE (the statement fails *inside* an
    /// otherwise-Ok response) and the field stays `false`. So this test seeds the
    /// row BEFORE running migrations, then asserts the precondition — the field is
    /// truly absent, not `false` — before exercising `list_all`.
    #[tokio::test]
    async fn list_all_includes_a_record_whose_vault_deleted_is_unset() {
        let db = surrealdb::engine::any::connect("mem://")
            .await
            .expect("mem db");
        db.use_ns("test").use_db("test").await.unwrap();

        // Seed before migrations: `npc` is schemaless here, so the row carries no
        // `vault_deleted` value at all.
        db.query(
            "CREATE campaign:c1 SET name = 'Shadows of Valdris', system = '5e', \
                 created_at = time::now(), updated_at = time::now(); \
             CREATE npc:n1 SET name = 'Seraphina Aldric', summary = 'Archivist', \
                 notes = 'GM notes', codex_article = 'Compiled.', \
                 created_at = time::now(), updated_at = time::now(); \
             RELATE campaign:c1->in_campaign->npc:n1;",
        )
        .await
        .expect("seed")
        .check()
        .expect("seed response");
        // Precondition, asserted BEFORE migrations: the row genuinely carries no
        // `vault_deleted` value — `DEFINE FIELD … DEFAULT false` never backfills.
        #[derive(serde::Deserialize)]
        struct Row {
            vault_deleted: Option<bool>,
        }
        let mut sel = db
            .query("SELECT vault_deleted FROM npc:n1")
            .await
            .expect("select field")
            .check()
            .expect("select response");
        let rows: Vec<Row> = sel.take(0).expect("take");
        assert_eq!(rows.len(), 1, "the seeded row must exist");
        assert!(
            rows[0].vault_deleted.is_none(),
            "precondition: vault_deleted must be genuinely unset, not false"
        );

        // `run_migrations` defines the field AND heals rows that predate it — an
        // unset non-optional field makes every later write to that record fail
        // (see `backfill_unset_fields`). The `!= true` read rule still stands
        // regardless: the backfill is deliberately non-fatal, so a row it could
        // not heal must still be treated as not-deleted rather than vanish.
        chronacle_db::run_migrations(&db).await.expect("migrations");

        let store = SurrealVaultRecordStore::new(db);
        let records = store.list_all().await.expect("list_all");
        assert_eq!(
            records.len(),
            1,
            "an unset vault_deleted must be treated as not-deleted"
        );
    }

    #[tokio::test]
    async fn list_all_returns_rule_entries_with_collection_scope() {
        let db = db().await;
        db.query(
            "CREATE collection:k1 SET name = 'D&D 5e Core', created_at = time::now(), updated_at = time::now(); \
             CREATE rule_entry:r1 SET collection = collection:k1, name = 'Grappling', \
                 category = 'procedure', body = 'Rules text.', compiled_at = time::now();",
        )
        .await
        .expect("seed")
        .check()
        .expect("seed response");
        let store = SurrealVaultRecordStore::new(db);

        let records = store.list_all().await.expect("list_all");
        let rule = records
            .iter()
            .find_map(|r| match r {
                VaultRecord::RuleEntry(x) => Some(x),
                _ => None,
            })
            .expect("one rule_entry");
        assert_eq!(rule.name, "Grappling");
        assert_eq!(rule.category, "procedure");
        assert!(
            matches!(&rule.collection, VaultScope::Collection { name, .. } if name == "D&D 5e Core")
        );
    }

    #[tokio::test]
    async fn synced_hash_round_trips_through_the_store() {
        let db = db().await;
        seed_campaign_npc(&db).await;
        let store = SurrealVaultRecordStore::new(db);
        let vref = VaultRef {
            table: "npc".into(),
            id: "n1".into(),
        };

        assert_eq!(store.get_synced_hash(&vref).await.expect("get"), None);

        // A hash above i64::MAX must survive — it is stored as a string.
        let big: u64 = u64::MAX - 7;
        store
            .set_synced_hash(&vref, "campaigns/c/entities/npc/a.md", big)
            .await
            .expect("set");
        assert_eq!(store.get_synced_hash(&vref).await.expect("get"), Some(big));

        store
            .set_synced_hash(&vref, "campaigns/c/entities/npc/a.md", 42)
            .await
            .expect("update");
        assert_eq!(store.get_synced_hash(&vref).await.expect("get"), Some(42));

        store.clear_synced_hash(&vref).await.expect("clear");
        assert_eq!(store.get_synced_hash(&vref).await.expect("get"), None);
    }

    #[tokio::test]
    async fn clear_all_synced_wipes_every_sync_state_row() {
        let db = db().await;
        seed_campaign_npc(&db).await;
        let store = SurrealVaultRecordStore::new(db);
        let vref = VaultRef {
            table: "npc".into(),
            id: "n1".into(),
        };
        store
            .set_synced_hash(&vref, "campaigns/c/entities/npc/a.md", 42)
            .await
            .expect("set");

        store.clear_all_synced().await.expect("clear all");
        assert_eq!(store.get_synced_hash(&vref).await.expect("get"), None);
    }

    #[tokio::test]
    async fn load_returns_none_for_a_missing_record() {
        let db = db().await;
        let store = SurrealVaultRecordStore::new(db);
        let vref = VaultRef {
            table: "npc".into(),
            id: "nope".into(),
        };
        assert!(store.load(&vref).await.expect("load").is_none());
    }

    #[tokio::test]
    async fn apply_gm_parts_updates_only_summary_and_notes() {
        let db = db().await;
        seed_campaign_npc(&db).await;
        let store = SurrealVaultRecordStore::new(db.clone());
        let vref = VaultRef {
            table: "npc".into(),
            id: "n1".into(),
        };

        store
            .apply_gm_parts(
                &vref,
                &chronacle_core::GmParts {
                    summary: Some("New summary.".into()),
                    notes: Some("Edited in Obsidian. [[Iron Tower]]".into()),
                    aliases: vec![],
                },
            )
            .await
            .expect("apply");

        let rec = store.load(&vref).await.expect("load").expect("exists");
        let chronacle_core::VaultRecord::Entity(e) = rec else {
            panic!("entity")
        };
        assert_eq!(e.summary.as_deref(), Some("New summary."));
        assert_eq!(
            e.notes.as_deref(),
            Some("Edited in Obsidian. [[Iron Tower]]")
        );
        assert_eq!(e.name, "Seraphina Aldric", "name is never applied inbound");
        assert_eq!(
            e.codex_article.as_deref(),
            Some("Compiled."),
            "article untouched"
        );
    }

    /// Pins the real `GmParts` contract (see its doc comment): a `None` field
    /// is a deletion, not "no opinion". A vault file with no `## Summary`
    /// section must clear a pre-existing summary, not preserve it.
    #[tokio::test]
    async fn apply_gm_parts_clears_a_field_absent_from_the_file() {
        let db = db().await;
        seed_campaign_npc(&db).await;
        let store = SurrealVaultRecordStore::new(db.clone());
        let vref = VaultRef {
            table: "npc".into(),
            id: "n1".into(),
        };

        // Precondition: the seeded npc has a summary.
        let before = store.load(&vref).await.expect("load").expect("exists");
        let chronacle_core::VaultRecord::Entity(e) = before else {
            panic!("entity")
        };
        assert_eq!(e.summary.as_deref(), Some("Archivist"));

        store
            .apply_gm_parts(
                &vref,
                &chronacle_core::GmParts {
                    summary: None,
                    notes: Some("kept".into()),
                    aliases: vec![],
                },
            )
            .await
            .expect("apply");

        let after = store.load(&vref).await.expect("load").expect("exists");
        let chronacle_core::VaultRecord::Entity(e) = after else {
            panic!("entity")
        };
        assert_eq!(
            e.summary, None,
            "a section absent from the file must clear the DB field"
        );
        assert_eq!(e.notes.as_deref(), Some("kept"));
    }

    /// The DB-round-trip half of the aliases seam: `apply_gm_parts` writes
    /// `aliases`, and `list_all`/`load` reads them back on the resulting
    /// `EntityRecord`. The export-side merge with `name` is `chronacle-vault`'s
    /// job, not this store's — this only proves the column round-trips.
    #[tokio::test]
    async fn apply_gm_parts_writes_aliases_on_an_entity() {
        let db = db().await;
        seed_campaign_npc(&db).await;
        let store = SurrealVaultRecordStore::new(db.clone());
        let vref = VaultRef {
            table: "npc".into(),
            id: "n1".into(),
        };

        store
            .apply_gm_parts(
                &vref,
                &chronacle_core::GmParts {
                    summary: Some("Archivist".into()),
                    notes: Some("kept".into()),
                    aliases: vec!["The Archivist".into(), "Seraphina".into()],
                },
            )
            .await
            .expect("apply");

        let rec = store.load(&vref).await.expect("load").expect("exists");
        let chronacle_core::VaultRecord::Entity(e) = rec else {
            panic!("entity")
        };
        assert_eq!(
            e.aliases,
            vec!["The Archivist".to_string(), "Seraphina".to_string()]
        );
    }

    #[tokio::test]
    async fn apply_gm_parts_writes_aliases_on_a_rule_entry() {
        let db = db().await;
        db.query(
            "CREATE collection:k1 SET name = 'D&D 5e Core', created_at = time::now(), updated_at = time::now(); \
             CREATE rule_entry:r1 SET collection = collection:k1, name = 'Grappling', \
                 category = 'procedure', body = 'Rules text.', compiled_at = time::now();",
        )
        .await
        .expect("seed")
        .check()
        .expect("seed response");
        let store = SurrealVaultRecordStore::new(db.clone());
        let vref = VaultRef {
            table: "rule_entry".into(),
            id: "r1".into(),
        };

        store
            .apply_gm_parts(
                &vref,
                &chronacle_core::GmParts {
                    summary: None,
                    notes: Some("kept".into()),
                    aliases: vec!["Grapple".into()],
                },
            )
            .await
            .expect("apply");

        let rec = store.load(&vref).await.expect("load").expect("exists");
        let chronacle_core::VaultRecord::RuleEntry(r) = rec else {
            panic!("rule_entry")
        };
        assert_eq!(r.aliases, vec!["Grapple".to_string()]);
    }

    #[tokio::test]
    async fn soft_delete_removes_the_record_from_list_all() {
        let db = db().await;
        seed_campaign_npc(&db).await;
        let store = SurrealVaultRecordStore::new(db);
        let vref = VaultRef {
            table: "npc".into(),
            id: "n1".into(),
        };
        store.soft_delete(&vref).await.expect("soft delete");
        assert!(store.list_all().await.expect("list").is_empty());
    }

    #[tokio::test]
    async fn soft_delete_removes_a_rule_entry_from_list_all() {
        let db = db().await;
        db.query(
            "CREATE collection:k1 SET name = 'D&D 5e Core', created_at = time::now(), updated_at = time::now(); \
             CREATE rule_entry:r1 SET collection = collection:k1, name = 'Grappling', \
                 category = 'procedure', body = 'Rules text.', compiled_at = time::now();",
        )
        .await
        .expect("seed")
        .check()
        .expect("seed response");
        let store = SurrealVaultRecordStore::new(db);
        let vref = VaultRef {
            table: "rule_entry".into(),
            id: "r1".into(),
        };
        store.soft_delete(&vref).await.expect("soft delete");
        assert!(store.list_all().await.expect("list").is_empty());
    }

    #[tokio::test]
    async fn set_conflict_round_trips_through_list_synced_without_a_base() {
        let db = db().await;
        seed_campaign_npc(&db).await;
        let store = SurrealVaultRecordStore::new(db);
        let vref = VaultRef {
            table: "npc".into(),
            id: "n1".into(),
        };

        store
            .set_conflict(&vref, "campaigns/c/entities/npc/a.md", true)
            .await
            .expect("set");
        let rows = store.list_synced().await.expect("list");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].conflict);
        assert_eq!(rows[0].synced_hash, None, "conflict-only row has no base");
        assert_eq!(rows[0].key, "campaigns/c/entities/npc/a.md");

        store
            .set_conflict(&vref, "campaigns/c/entities/npc/a.md", false)
            .await
            .expect("clear");
        assert!(!store.list_synced().await.expect("list")[0].conflict);
    }

    /// `set_conflict` UPSERTs the same row `set_synced_hash` writes. If it
    /// replaced the row instead of merging into it, flagging a record
    /// conflicted would silently destroy its merge base and corrupt every
    /// subsequent sync decision. Covers both call orders.
    #[tokio::test]
    async fn set_conflict_preserves_an_existing_synced_hash() {
        let db = db().await;
        seed_campaign_npc(&db).await;
        let store = SurrealVaultRecordStore::new(db);
        let vref = VaultRef {
            table: "npc".into(),
            id: "n1".into(),
        };
        let key = "campaigns/c/entities/npc/a.md";

        // hash first, then conflict: conflict must not wipe the base.
        store.set_synced_hash(&vref, key, 42).await.expect("set");
        store
            .set_conflict(&vref, key, true)
            .await
            .expect("conflict");
        let rows = store.list_synced().await.expect("list");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].conflict, "conflict flag must be set");
        assert_eq!(
            rows[0].synced_hash,
            Some(42),
            "set_conflict must preserve the existing merge base"
        );

        // conflict first, then hash: hash write must not clear the flag.
        store
            .set_conflict(&vref, key, false)
            .await
            .expect("clear conflict");
        store.set_synced_hash(&vref, key, 7).await.expect("set");
        store
            .set_conflict(&vref, key, true)
            .await
            .expect("conflict again");
        let rows = store.list_synced().await.expect("list");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].conflict, "conflict must survive a prior hash set");
        assert_eq!(rows[0].synced_hash, Some(7));
    }
}
