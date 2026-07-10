//! The three-way sync decision. Pure: a function of three content hashes.
//!
//! Timestamps are deliberately absent. `codex_service::compile.rs` updates
//! `codex_article` without touching `updated_at`, so an `mtime`-vs-`updated_at`
//! comparison would never re-export a recompiled article. And a timestamp delta
//! cannot distinguish "the file is a stale copy" from "the file has divergent
//! edits" — that needs a base.

/// What reconcile should do about one record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncAction {
    /// Both sides match the base. Nothing to do.
    NoOp,
    /// Both sides agree with each other but not with the base. Record the base.
    AdoptBase,
    /// The database changed; the file did not. Write the file.
    Export,
    /// The file changed; the database did not. Apply inbound. (Tranche 5.)
    Apply,
    /// Both changed, differently. Preserve both. (Tranche 5.)
    Conflict,
    /// We wrote this file once and it is gone. (Tranche 5.)
    SoftDelete,
}

/// Decide what to do, given the last-synced hash, the rendered-record hash, and
/// the file's content hash (`None` when the key is absent from the vault).
pub fn decide(base: Option<u64>, db: u64, file: Option<u64>) -> SyncAction {
    let Some(file) = file else {
        // No file. If we never wrote one, this is the first export. If we did,
        // the GM deleted it.
        return match base {
            None => SyncAction::Export,
            Some(_) => SyncAction::SoftDelete,
        };
    };

    // MUST precede the conflict branch: after a crash between write and
    // base-update, both sides differ from the base but agree with each other.
    if db == file {
        return if base == Some(db) {
            SyncAction::NoOp
        } else {
            SyncAction::AdoptBase
        };
    }

    match base {
        // An unmanaged file already claims this id and differs from us.
        None => SyncAction::Conflict,
        Some(base) => match (db == base, file == base) {
            (true, true) => unreachable!("db == file was handled above"),
            (false, true) => SyncAction::Export,
            (true, false) => SyncAction::Apply,
            (false, false) => SyncAction::Conflict,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Hash values are opaque; only their equality matters.
    const BASE: u64 = 100;
    const CHANGED: u64 = 200;
    const OTHER: u64 = 300;

    #[test]
    fn nothing_changed_is_a_noop() {
        assert_eq!(decide(Some(BASE), BASE, Some(BASE)), SyncAction::NoOp);
    }

    #[test]
    fn db_changed_and_file_untouched_exports() {
        assert_eq!(decide(Some(BASE), CHANGED, Some(BASE)), SyncAction::Export);
    }

    #[test]
    fn file_changed_and_db_untouched_applies() {
        assert_eq!(decide(Some(BASE), BASE, Some(CHANGED)), SyncAction::Apply);
    }

    #[test]
    fn both_changed_differently_is_a_conflict() {
        assert_eq!(
            decide(Some(BASE), CHANGED, Some(OTHER)),
            SyncAction::Conflict
        );
    }

    /// Crash recovery: the app died between `VaultStore::write` and the
    /// `synced_hash` update. Both sides diverge from the base, but they agree
    /// with each other — that is not a conflict, it is a stale base.
    #[test]
    fn both_changed_identically_adopts_the_base_and_never_conflicts() {
        assert_eq!(
            decide(Some(BASE), CHANGED, Some(CHANGED)),
            SyncAction::AdoptBase
        );
    }

    #[test]
    fn no_base_and_no_file_is_a_first_export() {
        assert_eq!(decide(None, CHANGED, None), SyncAction::Export);
    }

    #[test]
    fn no_base_but_an_identical_file_adopts_the_base() {
        // e.g. the GM restored a vault backup that already matches the DB.
        assert_eq!(decide(None, CHANGED, Some(CHANGED)), SyncAction::AdoptBase);
    }

    #[test]
    fn no_base_and_a_differing_file_is_a_conflict() {
        // An unmanaged pre-existing file already claims this id. Never clobber it.
        assert_eq!(decide(None, CHANGED, Some(OTHER)), SyncAction::Conflict);
    }

    #[test]
    fn a_base_with_no_file_is_a_soft_delete() {
        // We wrote it once; it is gone now. The GM deleted it in the vault.
        assert_eq!(decide(Some(BASE), BASE, None), SyncAction::SoftDelete);
        assert_eq!(decide(Some(BASE), CHANGED, None), SyncAction::SoftDelete);
    }

    /// The decision never consults a clock. This is a compile-time property —
    /// the signature takes three hashes — but assert the shape anyway.
    #[test]
    fn decide_is_total_over_every_combination() {
        for base in [None, Some(BASE)] {
            for file in [None, Some(BASE), Some(CHANGED)] {
                for db in [BASE, CHANGED] {
                    let _ = decide(base, db, file); // must not panic
                }
            }
        }
    }
}
