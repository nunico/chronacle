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
    let q = "LET $ents = (SELECT VALUE out FROM in_collection \
                 WHERE in = type::thing('collection', $cid));
             LET $stale = (SELECT VALUE id FROM $ents \
                 WHERE codex_stale != false OR codex_article = NONE);
             LET $rules = (SELECT VALUE id FROM rule_entry \
                 WHERE collection = type::thing('collection', $cid));
             LET $rstale = (SELECT VALUE id FROM rule_entry \
                 WHERE collection = type::thing('collection', $cid) AND stale = true);
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
        .take(4)
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
    use surrealdb::engine::local::{Db, Mem};
    use surrealdb::Surreal;

    async fn setup_db() -> Surreal<Db> {
        let db = Surreal::new::<Mem>(()).await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        chronacle_db::run_migrations(&db).await.unwrap();
        db
    }

    #[tokio::test]
    async fn status_counts_stale_unset_and_missing_articles() {
        let db = setup_db().await;
        db.query(
            "CREATE collection:`c1` SET name = 'World', description = NULL, \
                 created_at = time::now(), updated_at = time::now();
             CREATE npc:`fresh` SET name = 'Fresh', codex_stale = false, \
                 codex_article = 'compiled text';
             CREATE npc:`stale` SET name = 'Stale', codex_stale = true;
             CREATE npc:`legacy` SET name = 'Legacy';
             UPDATE npc:`legacy` UNSET codex_stale;
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
}
