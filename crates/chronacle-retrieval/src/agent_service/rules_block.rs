//! RULES context block: KNN over compiled `rule_entry` rows scoped to the
//! campaign's subscribed collections, rendered budget-capped for the prompt.
//!
//! `fetch_rules_context` is wired into `stream_response` (`mod.rs`) and its
//! output is threaded through `prompt::build_system_prompt`.

use serde::Deserialize;
use surrealdb::Connection;

use super::AgentError;

/// Top-k rule entries retrieved per question.
pub(super) const RULES_TOP_K: usize = 5;
/// Whole-block character budget — compiled rules must not starve chunk evidence.
pub(super) const RULES_BLOCK_BUDGET: usize = 4_000;
/// Per-entry body character budget.
pub(super) const RULE_BODY_BUDGET: usize = 1_200;

/// One page reference on a retrieved rule entry.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct RulePageRef {
    pub source_name: String,
    pub page_start: i64,
    pub page_end: i64,
}

/// One retrieved rule entry (subset of the `rule_entry` row).
#[derive(Debug, Clone, Deserialize)]
pub(super) struct RuleHit {
    pub name: String,
    pub category: String,
    pub body: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub page_refs: Vec<RulePageRef>,
}

/// Render retrieved rule entries as the RULES prompt block, honoring the
/// per-entry and whole-block character budgets. Empty input ⇒ empty string.
pub(super) fn format_rules_block(hits: &[RuleHit]) -> String {
    if hits.is_empty() {
        return String::new();
    }
    let mut out = String::from("COMPILED RULES (distilled from your rulebooks):\n\n");
    for h in hits {
        let pages = h
            .page_refs
            .iter()
            .map(|p| {
                if p.page_start == p.page_end {
                    format!("{} p.{}", p.source_name, p.page_start)
                } else {
                    format!("{} p.{}-{}", p.source_name, p.page_start, p.page_end)
                }
            })
            .collect::<Vec<_>>()
            .join("; ");
        let body: String = if h.body.chars().count() > RULE_BODY_BUDGET {
            let cut: String = h.body.chars().take(RULE_BODY_BUDGET).collect();
            format!("{cut}…")
        } else {
            h.body.clone()
        };
        let mut entry = format!("[{}] {} — {}\n{}\n", h.category, h.name, pages, body);
        if let Some(n) = h.notes.as_deref() {
            let n = n.trim();
            if !n.is_empty() {
                entry.push_str(&format!("GM table ruling: {n}\n"));
            }
        }
        entry.push('\n');
        if out.chars().count() + entry.chars().count() > RULES_BLOCK_BUDGET {
            break;
        }
        out.push_str(&entry);
    }
    out
}

/// KNN top-[`RULES_TOP_K`] rule entries across `collection_ids`, formatted as
/// the RULES prompt block. Returns an empty string when there are no
/// collections, no embedded entries, or no hits.
///
/// The collection filter is an inline explicit array (`collection IN [...]`)
/// placed *before* the KNN clause in `WHERE`: SurrealDB's MTREE `<|K|>`
/// operator silently returns zero rows when a non-KNN predicate is ANDed in
/// after it — ordering matters here, not just subquery-vs-array shape.
pub(super) async fn fetch_rules_context<C: Connection>(
    db: &surrealdb::Surreal<C>,
    collection_ids: &[String],
    query_vec: &[f32],
) -> Result<String, AgentError> {
    if collection_ids.is_empty() || query_vec.is_empty() {
        return Ok(String::new());
    }
    let cols = collection_ids
        .iter()
        .map(|c| format!("collection:`{}`", c.replace('`', "")))
        .collect::<Vec<_>>()
        .join(", ");
    let vec_str = query_vec
        .iter()
        .map(|f| f.to_string())
        .collect::<Vec<_>>()
        .join(",");
    // Collection filter must precede the KNN clause in `WHERE`: SurrealDB's
    // MTREE KNN operator silently returns zero rows when a non-KNN predicate
    // is ANDed in *after* it (order-sensitive, unlike a normal SQL `WHERE`).
    // `vault_deleted != true`, never `= false`: DEFAULT does not backfill
    // pre-migration rows. Both plain-field predicates are placed before the
    // KNN clause for the same reason as the collection filter.
    let sql = format!(
        "SELECT name, category, body, notes, page_refs, \
             vector::distance::knn() AS distance \
         FROM rule_entry \
         WHERE vault_deleted != true AND collection IN [{cols}] AND embedding <|{RULES_TOP_K}|> [{vec_str}] \
         ORDER BY distance ASC LIMIT {RULES_TOP_K}"
    );
    let mut resp = db
        .query(sql)
        .await
        .map_err(|e| AgentError::Retrieval(e.to_string()))?;
    let hits: Vec<RuleHit> = resp
        .take(0)
        .map_err(|e| AgentError::Retrieval(e.to_string()))?;
    Ok(format_rules_block(&hits))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(name: &str, body: &str, notes: Option<&str>) -> RuleHit {
        RuleHit {
            name: name.into(),
            category: "mechanic".into(),
            body: body.into(),
            notes: notes.map(String::from),
            page_refs: vec![RulePageRef {
                source_name: "PHB".into(),
                page_start: 14,
                page_end: 15,
            }],
        }
    }

    #[test]
    fn format_renders_category_pages_body_and_labeled_notes() {
        let out = format_rules_block(&[hit("Initiative", "Roll d20.", Some("We reroll ties"))]);
        assert!(out.contains("[mechanic] Initiative"));
        assert!(out.contains("PHB p.14-15"));
        assert!(out.contains("Roll d20."));
        assert!(out.contains("GM table ruling: We reroll ties"));
    }

    #[test]
    fn format_empty_input_is_empty_string() {
        assert_eq!(format_rules_block(&[]), "");
    }

    #[test]
    fn format_truncates_long_bodies_per_entry() {
        let long = "x".repeat(RULE_BODY_BUDGET + 500);
        let out = format_rules_block(&[hit("Big", &long, None)]);
        assert!(
            out.chars().count() < RULE_BODY_BUDGET + 300,
            "body must be excerpted"
        );
        assert!(out.contains('…'));
    }

    #[test]
    fn format_stops_at_block_budget() {
        let body = "y".repeat(RULE_BODY_BUDGET);
        let hits: Vec<RuleHit> = (0..10)
            .map(|i| hit(&format!("R{i}"), &body, None))
            .collect();
        let out = format_rules_block(&hits);
        assert!(
            out.chars().count() <= RULES_BLOCK_BUDGET + RULE_BODY_BUDGET,
            "block must cut off near the budget, got {}",
            out.chars().count()
        );
        assert!(
            !out.contains("R9"),
            "later entries must be dropped once over budget"
        );
    }

    #[test]
    fn single_page_refs_collapse() {
        let mut h = hit("One", "b", None);
        h.page_refs[0].page_end = 14;
        let out = format_rules_block(&[h]);
        assert!(out.contains("PHB p.14"));
        assert!(!out.contains("p.14-14"));
    }

    async fn setup_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        chronacle_db::run_migrations(&db).await.unwrap();
        db
    }

    /// Regression guard for the MTREE + subquery pitfall: the scoped KNN must
    /// use an inline explicit filter and actually return rows.
    #[tokio::test]
    async fn fetch_rules_context_knn_respects_collection_scope() {
        let db = setup_db().await;
        let mut in_vec = vec![0.0f32; 768];
        in_vec[0] = 1.0;
        let mut out_vec = vec![0.0f32; 768];
        out_vec[1] = 1.0;
        db.query(
            "CREATE collection:`ca` SET name='A', description=NULL, created_at=time::now(), updated_at=time::now();
             CREATE collection:`cb` SET name='B', description=NULL, created_at=time::now(), updated_at=time::now();
             CREATE rule_entry:`r1` SET collection=collection:`ca`, name='Initiative', category='mechanic',
                 body='Roll d20 and add DEX.', compiled_at=time::now(), stale=false,
                 page_refs=[{ source_name: 'PHB', page_start: 14, page_end: 15 }],
                 embedding=$va, embed_model='mock';
             CREATE rule_entry:`r2` SET collection=collection:`cb`, name='Stealth', category='ability',
                 body='Out-of-scope rule.', compiled_at=time::now(), stale=false,
                 embedding=$va, embed_model='mock';",
        )
        .bind(("va", in_vec.clone()))
        .await
        .unwrap()
        .check()
        .unwrap();

        let ctx = fetch_rules_context(&db, &["ca".to_string()], &in_vec)
            .await
            .unwrap();
        assert!(
            ctx.contains("Initiative"),
            "in-scope rule must be retrieved: {ctx}"
        );
        assert!(ctx.contains("PHB p.14-15"));
        assert!(
            !ctx.contains("Stealth"),
            "out-of-scope rule must be filtered: {ctx}"
        );

        let empty = fetch_rules_context(&db, &[], &in_vec).await.unwrap();
        assert_eq!(empty, "", "no collections ⇒ no block");
    }

    /// A soft-deleted rule entry (`vault_deleted = true`) must never be fed
    /// to the LLM as compiled-rules context, even when it is otherwise the
    /// closest KNN hit.
    #[tokio::test]
    async fn fetch_rules_context_excludes_a_soft_deleted_rule_entry() {
        let db = setup_db().await;
        let mut in_vec = vec![0.0f32; 768];
        in_vec[0] = 1.0;
        db.query(
            "CREATE collection:`ca` SET name='A', description=NULL, created_at=time::now(), updated_at=time::now();
             CREATE rule_entry:`deleted` SET collection=collection:`ca`, name='Deleted Rule', category='mechanic',
                 body='This was removed by the GM.', compiled_at=time::now(), stale=false,
                 vault_deleted=true,
                 embedding=$va, embed_model='mock';
             CREATE rule_entry:`live` SET collection=collection:`ca`, name='Live Rule', category='mechanic',
                 body='Still here.', compiled_at=time::now(), stale=false,
                 embedding=$va, embed_model='mock';",
        )
        .bind(("va", in_vec.clone()))
        .await
        .unwrap()
        .check()
        .unwrap();

        let ctx = fetch_rules_context(&db, &["ca".to_string()], &in_vec)
            .await
            .unwrap();
        assert!(
            !ctx.contains("Deleted Rule"),
            "soft-deleted rule entry must never reach the LLM: {ctx}"
        );
        assert!(ctx.contains("Live Rule"));
    }
}
