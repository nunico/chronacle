//! Dependency-inversion ports for Markdown vault sync (ADR-008).
//!
//! `chronacle-vault` owns the sync engine and depends only on these traits
//! and DTOs, never on a concrete filesystem or database client. Concrete
//! implementations live in `chronacle-providers` (`VaultStore`) and
//! `chronacle-domain` (`VaultRecordStore`).

/// A record's stable identity: table name + raw id, e.g. `("npc", "abc123")`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct VaultRef {
    pub table: String,
    pub id: String,
}

impl VaultRef {
    /// Parse a SurrealDB thing string (`"npc:abc123"`) into a `VaultRef`.
    ///
    /// Splits on the *first* colon — SurrealDB record ids may contain colons.
    pub fn parse(thing: &str) -> Option<VaultRef> {
        let (table, id) = thing.split_once(':')?;
        if table.is_empty() || id.is_empty() {
            return None;
        }
        Some(VaultRef {
            table: table.to_owned(),
            id: id.to_owned(),
        })
    }

    /// Render back to a SurrealDB thing string.
    pub fn to_thing(&self) -> String {
        format!("{}:{}", self.table, self.id)
    }
}

/// A vault key: a `/`-separated, POSIX-style path relative to the vault root.
///
/// Never an OS path. `LocalFsVaultStore` is the only thing that joins it to one.
pub type VaultKey = String;

/// Metadata about a stored vault file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultMetadata {
    pub mtime: std::time::SystemTime,
}

/// A change event surfaced by a `VaultWatcher`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultEvent {
    Upsert(VaultKey),
    Remove(VaultKey),
    Rescan,
}

/// Errors surfaced by a `VaultStore` implementation.
#[derive(Debug, thiserror::Error)]
pub enum VaultStoreError {
    #[error("I/O error ({kind:?}): {message}")]
    Io {
        kind: std::io::ErrorKind,
        message: String,
    },
    #[error("Not found: {0}")]
    NotFound(VaultKey),
    #[error("Invalid key: {0}")]
    InvalidKey(VaultKey),
}

/// Keyed blob I/O over a Markdown vault.
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
#[async_trait::async_trait]
pub trait VaultStore: Send + Sync {
    /// Read the file at `key` as UTF-8 text.
    async fn read(&self, key: &str) -> Result<String, VaultStoreError>;
    /// Write `content` to `key`, creating parent directories as needed.
    async fn write(&self, key: &str, content: &str) -> Result<(), VaultStoreError>;
    /// Delete the file at `key`.
    async fn delete(&self, key: &str) -> Result<(), VaultStoreError>;
    /// Recursive. Returns keys (not OS paths) under `prefix`, `.md` files only.
    async fn list(&self, prefix: &str) -> Result<Vec<VaultKey>, VaultStoreError>;
    /// Fetch metadata (currently just `mtime`) for `key`.
    async fn metadata(&self, key: &str) -> Result<VaultMetadata, VaultStoreError>;
}

/// Subscribes to filesystem change events for the vault.
///
/// Declared here so the sync service signature is stable; the implementation
/// is out of scope for this tranche (tranche 5).
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
#[async_trait::async_trait]
pub trait VaultWatcher: Send + Sync {
    /// Tranche 5. Declared here so the service signature is stable.
    async fn subscribe(&self) -> tokio::sync::mpsc::Receiver<VaultEvent>;
}

/// Fire-and-forget notification that a record changed and should be exported.
///
/// Producers depend on this and nothing else vault-shaped.
pub trait VaultOutbound: Send + Sync {
    fn enqueue(&self, target: VaultRef);
}

/// A no-op `VaultOutbound` used wherever vault sync is disabled.
///
/// Keeps producers `Option`-free.
pub struct NoopOutbound;

impl VaultOutbound for NoopOutbound {
    fn enqueue(&self, _: VaultRef) {}
}

/// The three record shapes the vault mirrors. One enum, not five method families.
#[derive(Debug, Clone, PartialEq)]
pub enum VaultRecord {
    Entity(EntityRecord),
    Session(SessionRecord),
    RuleEntry(RuleEntryRecord),
}

/// A campaign or global entity (NPC, location, faction, etc.).
#[derive(Debug, Clone, PartialEq)]
pub struct EntityRecord {
    /// `table` is the entity kind, e.g. `"npc"`.
    pub vref: VaultRef,
    pub name: String,
    pub summary: Option<String>,
    pub notes: Option<String>,
    pub codex_article: Option<String>,
    pub scope: VaultScope,
    /// RFC3339 timestamp.
    pub created_at: String,
    /// RFC3339 timestamp.
    pub updated_at: String,
}

/// A campaign session log.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionRecord {
    pub vref: VaultRef,
    pub session_number: i64,
    pub title: String,
    pub date_played: String,
    pub notes: String,
    /// Always `VaultScope::Campaign`.
    pub campaign: VaultScope,
    pub created_at: String,
    pub updated_at: String,
}

/// A compiled rulebook entry.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleEntryRecord {
    pub vref: VaultRef,
    pub name: String,
    pub category: String,
    pub body: String,
    pub notes: Option<String>,
    pub page_refs: Vec<RulePageRef>,
    /// Always `VaultScope::Collection`.
    pub collection: VaultScope,
    pub created_at: String,
    pub updated_at: String,
}

/// A page reference into a source rulebook.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RulePageRef {
    pub source_name: String,
    pub page_start: i64,
    pub page_end: i64,
}

/// The owning scope of a record, carrying both id and display name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VaultScope {
    Campaign { id: String, name: String },
    Collection { id: String, name: String },
}

/// GM-owned fields an inbound vault edit may update.
///
/// Deliberately narrow: never `name`, never a compiled article/body.
///
/// `GmParts` is built by parsing an *entire* vault file
/// (`markdown::split_body`), so it always describes the COMPLETE desired
/// state of these fields, not a diff. `None` means the corresponding section
/// is absent from the file — the GM deleted it — and `apply_gm_parts` CLEARS
/// the field in the database (`NULL` for entities/rule entries, `""` for a
/// session's non-nullable `notes`). `Some(s)` sets the field to `s`
/// (`Some("")` is indistinguishable from `None` for a session's `notes`).
///
/// There is no "leave unchanged" state. Never construct a `GmParts` from
/// anything other than a full parse of the file's current content — a
/// partially-populated value would silently delete data the GM never
/// touched.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GmParts {
    /// Entities only. Ignored for sessions and rule entries.
    pub summary: Option<String>,
    /// Entities, sessions, and rule entries.
    pub notes: Option<String>,
}

/// A single persisted `vault_sync_state` row, as read by `list_synced`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncedRow {
    pub vref: VaultRef,
    pub key: VaultKey,
    /// `None` when the row has never had a base hash (e.g. conflict-only).
    pub synced_hash: Option<u64>,
    pub conflict: bool,
}

/// Errors surfaced by a `VaultRecordStore` implementation.
#[derive(Debug, thiserror::Error)]
pub enum VaultRecordError {
    #[error("record store error: {0}")]
    Backend(String),
    #[error("not found: {0}")]
    NotFound(String),
}

/// Read/write access to the records the vault mirrors, plus the persisted
/// merge-base hash used for three-way sync decisions.
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
#[async_trait::async_trait]
pub trait VaultRecordStore: Send + Sync {
    /// Every syncable record, excluding soft-deleted ones (`vault_deleted != true`).
    async fn list_all(&self) -> Result<Vec<VaultRecord>, VaultRecordError>;
    /// Load a single record by its identity, if it exists.
    async fn load(&self, vref: &VaultRef) -> Result<Option<VaultRecord>, VaultRecordError>;
    /// Persisted merge base. `None` when the record has never synced.
    async fn get_synced_hash(&self, vref: &VaultRef) -> Result<Option<u64>, VaultRecordError>;
    /// Persist the merge base hash and the vault key the record synced to.
    async fn set_synced_hash(
        &self,
        vref: &VaultRef,
        key: &str,
        hash: u64,
    ) -> Result<(), VaultRecordError>;
    /// Clear the persisted merge base, e.g. after a soft delete.
    async fn clear_synced_hash(&self, vref: &VaultRef) -> Result<(), VaultRecordError>;
    /// Wipe every persisted merge base (all `vault_sync_state` rows).
    /// Used when the vault path changes: the new directory gets a fresh baseline.
    async fn clear_all_synced(&self) -> Result<(), VaultRecordError>;
    /// Re-persist a previously snapshotted set of sync-state rows verbatim.
    ///
    /// The inverse of `clear_all_synced`, and the reason a vault-path switch can
    /// be rolled back: the merge base is the one piece of sync state nothing can
    /// re-derive. Once it is gone, a file the GM edited outside the app is
    /// indistinguishable from a file that never synced, so the next reconcile
    /// reads it as a fresh `Conflict` rather than a known divergence.
    async fn restore_synced(&self, rows: &[SyncedRow]) -> Result<(), VaultRecordError>;
    /// Every persisted sync-state row. One query per reconcile pass; also
    /// powers the orphan sweep (rows whose record no longer syncs).
    async fn list_synced(&self) -> Result<Vec<SyncedRow>, VaultRecordError>;
    /// Apply GM-owned fields inbound. Entities: summary + notes (+ wikilink
    /// resync, codex_stale). Sessions and rule entries: notes only.
    ///
    /// `parts` must describe the complete desired state — see the `GmParts`
    /// doc comment. A `None` field is written as a clear/delete, not skipped.
    async fn apply_gm_parts(
        &self,
        vref: &VaultRef,
        parts: &GmParts,
    ) -> Result<(), VaultRecordError>;
    /// Set `vault_deleted = true`. The record disappears from `list_all`.
    async fn soft_delete(&self, vref: &VaultRef) -> Result<(), VaultRecordError>;
    /// Mark or clear the frozen-conflict flag for a record's row (UPSERT).
    async fn set_conflict(
        &self,
        vref: &VaultRef,
        key: &str,
        in_conflict: bool,
    ) -> Result<(), VaultRecordError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_ref_parses_a_thing_string() {
        let r = VaultRef::parse("npc:abc123").expect("parse");
        assert_eq!(r.table, "npc");
        assert_eq!(r.id, "abc123");
    }

    #[test]
    fn vault_ref_round_trips_through_to_thing() {
        let r = VaultRef {
            table: "rule_entry".into(),
            id: "xyz".into(),
        };
        assert_eq!(r.to_thing(), "rule_entry:xyz");
        assert_eq!(VaultRef::parse(&r.to_thing()), Some(r));
    }

    #[test]
    fn vault_ref_rejects_a_string_without_a_colon() {
        assert_eq!(VaultRef::parse("npc"), None);
    }

    #[test]
    fn vault_ref_keeps_colons_inside_the_id() {
        // SurrealDB ids may themselves contain a colon; split once, on the first.
        let r = VaultRef::parse("npc:a:b").expect("parse");
        assert_eq!(r.table, "npc");
        assert_eq!(r.id, "a:b");
    }
}
