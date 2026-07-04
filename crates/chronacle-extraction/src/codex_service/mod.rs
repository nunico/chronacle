//! Codex service — the compiled-world-model layer (ADR-009).
//!
//! A2b skeleton: staleness marking and lint recording. Compilation (B1),
//! rules (B2), and proposals (C1) extend this module in later PRs.

use surrealdb::Connection;

mod compile;
mod prompts;
pub mod status;

#[cfg(test)]
mod compile_tests;

// Re-exported for future callers (e.g. a later PR's per-field re-embed
// trigger); not yet consumed outside `compile.rs` itself.
#[allow(unused_imports)]
pub(crate) use compile::embed_entity_with_article;
pub use compile::{compile_collection, compile_entity};
pub use status::{codex_status, CodexStatus};

/// Errors from codex compilation.
#[derive(Debug, thiserror::Error)]
pub enum CodexError {
    #[error("LLM error: {0}")]
    Llm(String),
    #[error("Database error: {0}")]
    Db(String),
    #[error("Embedding error: {0}")]
    Embedding(String),
}

/// Compile progress phases (serde snake_case, mirrors ExtractionPhase).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexPhase {
    Resolving,
    Compiling,
    Embedding,
    Done,
    Empty,
}

/// Progress payload for the `codex-progress` Tauri event.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CompileProgress {
    pub phase: CodexPhase,
    pub detail: String,
    pub compiled: usize,
    pub total: usize,
}

/// Result of a collection compile run.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CompileResult {
    pub articles_compiled: usize,
    /// Entities still needing compile after the per-run cap. Counts entities
    /// still flagged stale after the cap; entities skipped mid-run for lack
    /// of passages are not re-counted here — the next run picks them up.
    pub remaining_stale: usize,
}

/// Per-run cap on compiled entities (cost control; mirrors MAX_ENRICH).
pub const MAX_COMPILE_PER_RUN: usize = 50;

/// Mark one entity's codex article as stale (needs recompilation).
///
/// Producers: extraction touching an entity, user edits to summary/notes,
/// session-note mentions (C1). Cleared by the compiler (B1).
pub async fn mark_entity_stale<C: Connection>(
    db: &surrealdb::Surreal<C>,
    table: &str,
    id: &str,
) -> Result<(), String> {
    db.query("UPDATE type::thing($table, $id) SET codex_stale = true")
        .bind(("table", table.to_owned()))
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| format!("Failed to mark entity stale: {e}"))?;
    Ok(())
}

/// Record a lint finding for the maintenance inbox (C2 adds the UI).
///
/// `payload` shape depends on `kind`; shapes are documented in
/// `002_wiki_layer.surql`.
pub async fn record_lint<C: Connection>(
    db: &surrealdb::Surreal<C>,
    kind: &str,
    payload: serde_json::Value,
) -> Result<(), String> {
    db.query("CREATE lint_finding SET kind = $kind, payload = $payload")
        .bind(("kind", kind.to_owned()))
        .bind(("payload", payload))
        .await
        .map_err(|e| format!("Failed to record lint finding: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use surrealdb::engine::local::{Db, Mem};
    use surrealdb::Surreal;

    async fn setup_db() -> Surreal<Db> {
        let db = Surreal::new::<Mem>(()).await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        chronacle_db::run_migrations(&db).await.unwrap();
        db
    }

    #[derive(Deserialize)]
    struct CountRow {
        count: i64,
    }

    async fn count(db: &Surreal<Db>, q: &str) -> i64 {
        let mut resp = db.query(q).await.unwrap();
        let rows: Vec<CountRow> = resp.take(0).unwrap();
        rows.first().map(|r| r.count).unwrap_or(0)
    }

    #[tokio::test]
    async fn mark_entity_stale_sets_the_flag() {
        let db = setup_db().await;
        db.query("CREATE npc:`n1` SET name = 'Mira'").await.unwrap();
        mark_entity_stale(&db, "npc", "n1").await.unwrap();
        assert_eq!(
            count(
                &db,
                "SELECT count() FROM npc WHERE codex_stale = true GROUP ALL"
            )
            .await,
            1
        );
    }

    #[tokio::test]
    async fn record_lint_creates_unresolved_finding() {
        let db = setup_db().await;
        record_lint(
            &db,
            "scope_violation",
            serde_json::json!({ "from": "npc:a", "to": "npc:b" }),
        )
        .await
        .unwrap();
        assert_eq!(
            count(
                &db,
                "SELECT count() FROM lint_finding WHERE kind = 'scope_violation' \
                   AND resolved_at = NONE GROUP ALL"
            )
            .await,
            1
        );
        // The payload must round-trip as a structured object, not a blob or
        // an empty object: assert on fields inside it. (`from`/`to` are
        // SurrealQL keywords, hence the backticks.)
        assert_eq!(
            count(
                &db,
                "SELECT count() FROM lint_finding WHERE payload.`from` = 'npc:a' \
                   AND payload.`to` = 'npc:b' GROUP ALL"
            )
            .await,
            1
        );
    }
}
