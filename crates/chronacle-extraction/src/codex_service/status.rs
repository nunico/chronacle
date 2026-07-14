//! Codex staleness status for a collection (drives the UI badges).

use serde::Serialize;
use surrealdb::Connection;

/// Compile-staleness summary for one collection.
#[derive(Debug, Clone, Serialize)]
pub struct CodexStatus {
    pub stale_entities: usize,
    pub total_entities: usize,
    pub rules_stale: usize,
    pub rule_entries: usize,
}

/// Count entities needing compile (stale, unset-stale, or article-less) and
/// rule-entry staleness for `collection_id`.
///
/// Unset `codex_stale` (pre-migration rows) counts as stale: SurrealDB
/// evaluates `NONE != false` as true, which is exactly the semantics we want.
pub async fn codex_status<C: Connection>(
    db: &surrealdb::Surreal<C>,
    collection_id: &str,
) -> Result<CodexStatus, String> {
    // `vault_deleted != true`, never `= false`, on every branch below: DEFAULT
    // does not backfill pre-migration rows, and a soft-deleted entity or rule
    // entry must never count toward the compile-status badges.
    let q = "LET $all_ents = (SELECT VALUE out FROM in_collection \
                 WHERE in = type::thing('collection', $cid));
             LET $ents = (SELECT VALUE id FROM $all_ents WHERE vault_deleted != true);
             LET $stale = (SELECT VALUE id FROM $ents \
                 WHERE codex_stale != false OR codex_article = NONE);
             LET $rules = (SELECT VALUE id FROM rule_entry \
                 WHERE collection = type::thing('collection', $cid) AND vault_deleted != true);
             LET $rstale = (SELECT VALUE id FROM rule_entry \
                 WHERE collection = type::thing('collection', $cid) AND stale = true AND vault_deleted != true);
             RETURN { total: array::len($ents), stale: array::len($stale), \
                      rules: array::len($rules), rules_stale: array::len($rstale) };";
    #[derive(serde::Deserialize)]
    struct Row {
        total: usize,
        stale: usize,
        rules: usize,
        rules_stale: usize,
    }
    let mut resp = db
        .query(q)
        .bind(("cid", collection_id.to_owned()))
        .await
        .map_err(|e| format!("Failed to query codex status: {e}"))?;
    let row: Option<Row> = resp
        .take(5)
        .map_err(|e| format!("Failed to parse codex status: {e}"))?;
    let row = row.ok_or_else(|| "codex status query returned nothing".to_string())?;
    Ok(CodexStatus {
        stale_entities: row.stale,
        total_entities: row.total,
        rules_stale: row.rules_stale,
        rule_entries: row.rules,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn status_counts_stale_unset_and_missing_articles() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        // A row created BEFORE the migration defines the codex fields: this is the
        // genuine pre-migration state (field absent → reads as NONE; DEFAULT only
        // applies at create time and migrations never backfill).
        db.query(
            "CREATE npc:`legacy` SET name = 'Legacy', summary = NULL, notes = NULL, \
                 created_at = time::now(), updated_at = time::now()",
        )
        .await
        .unwrap()
        .check()
        .unwrap();
        chronacle_db::run_migrations(&db).await.unwrap();

        db.query(
            "CREATE collection:`c1` SET name = 'World', description = NULL, \
                 created_at = time::now(), updated_at = time::now();
             CREATE npc:`fresh` SET name = 'Fresh', codex_stale = false, \
                 codex_article = 'compiled text';
             CREATE npc:`stale` SET name = 'Stale', codex_stale = true;
             RELATE collection:`c1`->in_collection->npc:`fresh` SET created_at = time::now();
             RELATE collection:`c1`->in_collection->npc:`stale` SET created_at = time::now();
             RELATE collection:`c1`->in_collection->npc:`legacy` SET created_at = time::now();
             CREATE rule_entry SET collection = collection:`c1`, name = 'Initiative', \
                 category = 'mechanic', body = 'b', compiled_at = time::now(), stale = true;",
        )
        .await
        .unwrap()
        .check()
        .unwrap();

        let s = codex_status(&db, "c1").await.unwrap();
        assert_eq!(s.total_entities, 3);
        assert_eq!(
            s.stale_entities, 2,
            "stale flag AND unset flag both count as needing compile"
        );
        assert_eq!(s.rules_stale, 1);
        assert_eq!(s.rule_entries, 1);
    }

    /// A soft-deleted entity or rule entry must never count toward the
    /// compile-status badges — it is invisible everywhere in the app.
    #[tokio::test]
    async fn status_excludes_soft_deleted_entities_and_rule_entries() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        chronacle_db::run_migrations(&db).await.unwrap();

        db.query(
            "CREATE collection:`c1` SET name = 'World', description = NULL, \
                 created_at = time::now(), updated_at = time::now();
             CREATE npc:`fresh` SET name = 'Fresh', codex_stale = false, \
                 codex_article = 'compiled text';
             CREATE npc:`gone` SET name = 'Gone', codex_stale = true, vault_deleted = true;
             RELATE collection:`c1`->in_collection->npc:`fresh` SET created_at = time::now();
             RELATE collection:`c1`->in_collection->npc:`gone` SET created_at = time::now();
             CREATE rule_entry:`live` SET collection = collection:`c1`, name = 'Initiative', \
                 category = 'mechanic', body = 'b', compiled_at = time::now(), stale = true;
             CREATE rule_entry:`removed` SET collection = collection:`c1`, name = 'Old Rule', \
                 category = 'mechanic', body = 'b', compiled_at = time::now(), stale = true, \
                 vault_deleted = true;",
        )
        .await
        .unwrap()
        .check()
        .unwrap();

        let s = codex_status(&db, "c1").await.unwrap();
        assert_eq!(s.total_entities, 1, "the soft-deleted npc must not count");
        assert_eq!(
            s.rule_entries, 1,
            "the soft-deleted rule entry must not count"
        );
        assert_eq!(
            s.rules_stale, 1,
            "the soft-deleted rule entry must not count as stale either"
        );
    }
}
