//! Retrieval context assembly — resolving subscribed collections, gathering
//! campaign/collection entity notes, and formatting retrieved chunks into the
//! prompt's reference block.

use surrealdb::Connection;

use super::AgentError;

/// Resolve the collection IDs that a campaign is subscribed to.
///
/// Queries the `subscribes_to` relation for the given `campaign_id` and
/// returns the bare IDs (no `table:` prefix) of all subscribed collections.
/// Returns an empty `Vec` when the campaign has no subscriptions.
pub async fn resolve_collection_ids<C>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
) -> Result<Vec<String>, AgentError>
where
    C: Connection,
{
    let mut response = db
        .query("SELECT out FROM subscribes_to WHERE in = type::thing('campaign', $id)")
        .bind(("id", campaign_id.to_owned()))
        .await
        .map_err(|e| AgentError::Db(e.to_string()))?;

    #[derive(serde::Deserialize)]
    struct Row {
        out: surrealdb::sql::Thing,
    }

    let rows: Vec<Row> = response
        .take(0)
        .map_err(|e| AgentError::Db(e.to_string()))?;

    Ok(rows.into_iter().map(|r| r.out.id.to_raw()).collect())
}

/// Max characters of an entity/session note included in the context block.
/// Notes can be long; we include a leading excerpt so the LLM sees the GM's
/// own prose without letting a single entity dominate the prompt budget.
const NOTES_EXCERPT_LEN: usize = 280;

/// Format a notes field as a single-line context excerpt, or `None` when empty.
///
/// Newlines are collapsed to spaces so each entity stays on its own line, and
/// the text is truncated on a char boundary with an ellipsis when over budget.
fn notes_excerpt(notes: Option<&str>) -> Option<String> {
    let trimmed = notes?.trim();
    if trimmed.is_empty() {
        return None;
    }
    let collapsed = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= NOTES_EXCERPT_LEN {
        Some(collapsed)
    } else {
        let truncated: String = collapsed.chars().take(NOTES_EXCERPT_LEN).collect();
        Some(format!("{truncated}…"))
    }
}

/// Query entity tables for a campaign (and optionally subscribed collections)
/// and format them as a context block.
///
/// Campaign-scoped entities are always included in full. Collection-scoped
/// entities are retrieved via MTREE KNN search when `query_embedding` is
/// `Some`, falling back to a full scan otherwise (tests, mock provider).
///
/// Returns an empty string when no entities are found.
pub async fn fetch_entity_context<C: Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
    collection_ids: &[String],
    query_embedding: Option<&[f32]>,
) -> Result<String, AgentError> {
    #[derive(serde::Deserialize)]
    struct BasicRow {
        name: String,
        summary: Option<String>,
        notes: Option<String>,
    }

    #[derive(serde::Deserialize)]
    struct PcRow {
        name: String,
        summary: Option<String>,
        notes: Option<String>,
        player_name: Option<String>,
        character_class: Option<String>,
        character_level: Option<i64>,
        status: Option<String>,
    }

    #[derive(serde::Deserialize)]
    struct EventRow {
        name: String,
        summary: Option<String>,
        notes: Option<String>,
        date_start: Option<String>,
        date_end: Option<String>,
    }

    #[derive(serde::Deserialize)]
    struct SessionRow {
        title: String,
        notes: Option<String>,
        date_played: Option<String>,
        session_number: Option<i64>,
    }

    // ── Campaign entities (always full scan) ─────────────────────────────────
    let mut resp = db
        .query("SELECT name, summary, notes, player_name, character_class, character_level, status FROM player_character WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes FROM npc WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes FROM location WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes FROM faction WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes FROM creature WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes FROM item WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes, date_start, date_end FROM event WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes FROM misc WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT title, notes, date_played, session_number FROM session WHERE campaign = type::thing('campaign', $cid) ORDER BY session_number ASC")
        .bind(("cid", campaign_id.to_owned()))
        .await
        .map_err(|e| AgentError::Db(e.to_string()))?;

    let pcs: Vec<PcRow> = resp.take(0).map_err(|e| AgentError::Db(e.to_string()))?;
    let npcs: Vec<BasicRow> = resp.take(1).map_err(|e| AgentError::Db(e.to_string()))?;
    let locations: Vec<BasicRow> = resp.take(2).map_err(|e| AgentError::Db(e.to_string()))?;
    let factions: Vec<BasicRow> = resp.take(3).map_err(|e| AgentError::Db(e.to_string()))?;
    let creatures: Vec<BasicRow> = resp.take(4).map_err(|e| AgentError::Db(e.to_string()))?;
    let items: Vec<BasicRow> = resp.take(5).map_err(|e| AgentError::Db(e.to_string()))?;
    let events: Vec<EventRow> = resp.take(6).map_err(|e| AgentError::Db(e.to_string()))?;
    let misc: Vec<BasicRow> = resp.take(7).map_err(|e| AgentError::Db(e.to_string()))?;
    let sessions: Vec<SessionRow> = resp.take(8).map_err(|e| AgentError::Db(e.to_string()))?;

    // ── Collection entities (top-k per table via MTREE, full scan as fallback) ─
    // Retrieved as a flat Vec<BasicRow> across all tables for the context block.
    let mut col_entities: Vec<(String, BasicRow)> = Vec::new(); // (kind, row)
    if !collection_ids.is_empty() {
        // Build a WHERE clause that matches entities in any of the given collections.
        // Each `collection:id->in_collection` traversal returns the entity IDs for
        // that collection; OR-ing them covers multiple subscriptions.
        let col_filter: String = collection_ids
            .iter()
            .map(|cid| {
                // Graph-traversal form: from the entity, walk back along the
                // in_collection edge to its collection(s) and test membership.
                // NOTE: a `id IN (SELECT ...)` subquery does NOT compose with the
                // MTREE KNN operator (`embedding <|K|> $vec`) — the combination
                // silently returns zero rows. The traversal form composes; the
                // explicit-array form would too. See the regression test
                // `fetch_entity_context_knn_over_collection_executes`.
                let safe = cid.replace('\'', "\\'");
                format!("<-in_collection<-collection CONTAINS type::thing('collection', '{safe}')")
            })
            .collect::<Vec<_>>()
            .join(" OR ");

        for table in &[
            "npc",
            "location",
            "faction",
            "creature",
            "item",
            "event",
            "player_character",
            "misc",
        ] {
            let sql = if let Some(qv) = query_embedding {
                // MTREE KNN: order by cosine distance, top 10 per table.
                let vec_str = qv
                    .iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                // KNN pattern (see providers/vector_store.rs): the `<|K|>`
                // operator must live in WHERE to activate the index; ordering is
                // by the computed distance. Putting `embedding <|K|> $vec` in
                // ORDER BY is rejected by SurrealDB ("missing order idiom
                // `embedding` in statement selection").
                format!(
                    "SELECT name, summary, notes, vector::distance::knn() AS distance \
                     FROM {table} \
                     WHERE embedding <|10|> [{vec_str}] AND ({col_filter}) \
                     ORDER BY distance ASC LIMIT 10"
                )
            } else {
                // Full scan fallback (no embedding provider / test paths).
                format!("SELECT name, summary, notes FROM {table} WHERE {col_filter} LIMIT 50")
            };
            let mut r = db
                .query(sql)
                .await
                .map_err(|e| AgentError::Db(e.to_string()))?;
            let rows: Vec<BasicRow> = r.take(0).map_err(|e| AgentError::Db(e.to_string()))?;
            for row in rows {
                col_entities.push((table.to_string(), row));
            }
        }
    }

    if pcs.is_empty()
        && npcs.is_empty()
        && locations.is_empty()
        && factions.is_empty()
        && creatures.is_empty()
        && items.is_empty()
        && events.is_empty()
        && misc.is_empty()
        && sessions.is_empty()
        && col_entities.is_empty()
    {
        return Ok(String::new());
    }

    let mut out = String::from("Campaign notes (your GM records):\n");

    if !pcs.is_empty() {
        out.push('\n');
        for r in &pcs {
            out.push_str(&format!("[player_character] {}", r.name));
            if let Some(p) = &r.player_name {
                out.push_str(&format!(" · Player: {p}"));
            }
            if let Some(c) = &r.character_class {
                out.push_str(&format!(" · Class: {c}"));
            }
            if let Some(l) = r.character_level {
                out.push_str(&format!(" · Level: {l}"));
            }
            if let Some(s) = &r.status {
                out.push_str(&format!(" · Status: {s}"));
            }
            if let Some(s) = &r.summary {
                if !s.trim().is_empty() {
                    out.push_str(&format!(" · {s}"));
                }
            }
            if let Some(n) = notes_excerpt(r.notes.as_deref()) {
                out.push_str(&format!(" · Notes: {n}"));
            }
            out.push('\n');
        }
    }

    for (rows, kind) in [
        (&npcs, "npc"),
        (&locations, "location"),
        (&factions, "faction"),
        (&creatures, "creature"),
        (&items, "item"),
    ] {
        if !rows.is_empty() {
            out.push('\n');
            for r in rows {
                out.push_str(&format!("[{kind}] {}", r.name));
                if let Some(s) = &r.summary {
                    if !s.trim().is_empty() {
                        out.push_str(&format!(" · {s}"));
                    }
                }
                if let Some(n) = notes_excerpt(r.notes.as_deref()) {
                    out.push_str(&format!(" · Notes: {n}"));
                }
                out.push('\n');
            }
        }
    }

    if !events.is_empty() {
        out.push('\n');
        for r in &events {
            out.push_str(&format!("[event] {}", r.name));
            match (&r.date_start, &r.date_end) {
                (Some(s), Some(e)) if !s.trim().is_empty() && !e.trim().is_empty() => {
                    out.push_str(&format!(" · {s} → {e}"));
                }
                (Some(s), _) if !s.trim().is_empty() => {
                    out.push_str(&format!(" · {s}"));
                }
                _ => {}
            }
            if let Some(s) = &r.summary {
                if !s.trim().is_empty() {
                    out.push_str(&format!(" · {s}"));
                }
            }
            if let Some(n) = notes_excerpt(r.notes.as_deref()) {
                out.push_str(&format!(" · Notes: {n}"));
            }
            out.push('\n');
        }
    }

    if !misc.is_empty() {
        out.push('\n');
        for r in &misc {
            out.push_str(&format!("[misc] {}", r.name));
            if let Some(s) = &r.summary {
                if !s.trim().is_empty() {
                    out.push_str(&format!(" · {s}"));
                }
            }
            if let Some(n) = notes_excerpt(r.notes.as_deref()) {
                out.push_str(&format!(" · Notes: {n}"));
            }
            out.push('\n');
        }
    }

    if !sessions.is_empty() {
        out.push('\n');
        for r in &sessions {
            match r.session_number {
                Some(num) => out.push_str(&format!("[session {num}] {}", r.title)),
                None => out.push_str(&format!("[session] {}", r.title)),
            }
            if let Some(d) = &r.date_played {
                if !d.trim().is_empty() {
                    out.push_str(&format!(" · {d}"));
                }
            }
            if let Some(n) = notes_excerpt(r.notes.as_deref()) {
                out.push_str(&format!(" · Notes: {n}"));
            }
            out.push('\n');
        }
    }

    // ── Collection entities section ──────────────────────────────────────────
    if !col_entities.is_empty() {
        out.push_str("\nCollection knowledge (from subscribed rulebooks):\n");
        for (kind, r) in &col_entities {
            out.push_str(&format!("[{kind}] {}", r.name));
            if let Some(s) = &r.summary {
                if !s.trim().is_empty() {
                    out.push_str(&format!(" · {s}"));
                }
            }
            if let Some(n) = notes_excerpt(r.notes.as_deref()) {
                out.push_str(&format!(" · Notes: {n}"));
            }
            out.push('\n');
        }
    }

    Ok(out)
}

/// Build a context block from search results for the LLM prompt.
pub(super) fn build_context(results: &[crate::providers::vector_store::SearchResult]) -> String {
    if results.is_empty() {
        return String::new();
    }
    let mut ctx = String::from("Relevant source material:\n\n");
    for (i, r) in results.iter().enumerate() {
        let source = if r.source_name.is_empty() {
            &r.source_id
        } else {
            &r.source_name
        };
        ctx.push_str(&format!(
            "[{i}] Source: \"{source}\", p. {}-{} — \"{}\"\n{}\n\n",
            r.page_start, r.page_end, r.section_heading, r.text
        ));
    }
    ctx
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Context building tests ───────────────────────────────────

    #[test]
    fn test_build_context_empty() {
        let ctx = build_context(&[]);
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_build_context_with_results() {
        use crate::providers::vector_store::SearchResult;

        let results = vec![SearchResult {
            chunk_id: "chunk-1".into(),
            source_id: "source:abc".into(),
            source_name: "PHB.pdf".into(),
            text: "A fighter can use Action Surge once per rest.".into(),
            page_start: 72,
            page_end: 72,
            section_heading: "Fighter Class Features".into(),
            source_type: "rules".into(),
            distance: 0.15,
        }];

        let ctx = build_context(&results);
        assert!(!ctx.is_empty());
        assert!(ctx.contains("PHB.pdf"));
        assert!(ctx.contains("p. 72-72"));
        assert!(ctx.contains("Action Surge"));
    }

    #[test]
    fn notes_excerpt_collapses_and_truncates() {
        assert_eq!(notes_excerpt(None), None);
        assert_eq!(notes_excerpt(Some("   ")), None);
        assert_eq!(
            notes_excerpt(Some("line one\n\nline  two")),
            Some("line one line two".to_string())
        );
        let long = "x ".repeat(400); // 400 single-char words
        let out = notes_excerpt(Some(&long)).unwrap();
        assert!(out.ends_with('…'), "expected ellipsis: {out}");
        assert_eq!(out.chars().count(), NOTES_EXCERPT_LEN + 1);
    }

    // ── Collection resolution tests ──────────────────────────────

    #[tokio::test]
    async fn resolve_collection_ids_returns_subscribed_ids() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        db.query(
            "CREATE collection SET id='col1', name='C1', created_at=time::now(), updated_at=time::now(); \
             CREATE collection SET id='col2', name='C2', created_at=time::now(), updated_at=time::now(); \
             CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()"
        ).await.unwrap();
        db.query(
            "LET $in = type::thing('campaign','camp1');
             LET $out1 = type::thing('collection','col1');
             LET $out2 = type::thing('collection','col2');
             RELATE $in->subscribes_to->$out1 SET created_at=time::now();
             RELATE $in->subscribes_to->$out2 SET created_at=time::now()",
        )
        .await
        .unwrap();

        let ids = resolve_collection_ids(&db, "camp1").await.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"col1".to_string()));
        assert!(ids.contains(&"col2".to_string()));
    }

    #[tokio::test]
    async fn resolve_collection_ids_empty_for_no_subscriptions() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        let ids = resolve_collection_ids(&db, "camp1").await.unwrap();
        assert!(ids.is_empty());
    }

    #[tokio::test]
    async fn fetch_entity_context_returns_empty_when_no_entities() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
        assert!(result.is_empty(), "expected empty string, got: {result:?}");
    }

    #[tokio::test]
    async fn fetch_entity_context_includes_player_character_fields() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(
            "CREATE player_character SET id='pc1', \
             name='Nazirdijan', player_name='Nico', character_class='Wizard', \
             character_level=5, status='active', summary=NULL, notes=NULL, \
             created_at=time::now(), updated_at=time::now(); \
             LET $src = type::thing('campaign','camp1'); \
             LET $dst = type::thing('player_character','pc1'); \
             RELATE $src->in_campaign->$dst SET created_at = time::now()",
        )
        .await
        .unwrap();

        let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
        assert!(
            result.contains("[player_character] Nazirdijan"),
            "missing entity line: {result}"
        );
        assert!(
            result.contains("Player: Nico"),
            "missing player_name: {result}"
        );
        assert!(result.contains("Class: Wizard"), "missing class: {result}");
        assert!(result.contains("Level: 5"), "missing level: {result}");
        assert!(
            result.contains("Status: active"),
            "missing status: {result}"
        );
    }

    #[tokio::test]
    async fn fetch_entity_context_omits_empty_sections() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(
            "CREATE npc SET id='npc1', \
             name='Aldric the Smith', summary='village blacksmith', notes=NULL, \
             created_at=time::now(), updated_at=time::now(); \
             LET $src = type::thing('campaign','camp1'); \
             LET $dst = type::thing('npc','npc1'); \
             RELATE $src->in_campaign->$dst SET created_at = time::now()",
        )
        .await
        .unwrap();

        let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
        assert!(
            result.contains("[npc] Aldric the Smith"),
            "missing npc: {result}"
        );
        assert!(
            result.contains("village blacksmith"),
            "missing summary: {result}"
        );
        assert!(
            !result.contains("[player_character]"),
            "unexpected PC section: {result}"
        );
        assert!(
            !result.contains("[location]"),
            "unexpected location section: {result}"
        );
    }

    #[tokio::test]
    async fn fetch_entity_context_includes_event_dates() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(
            "CREATE event SET id='ev1', \
             name='Battle of Irongate', date_start='Year 312', date_end='Year 313', \
             summary=NULL, notes=NULL, is_ongoing=false, \
             created_at=time::now(), updated_at=time::now(); \
             LET $src = type::thing('campaign','camp1'); \
             LET $dst = type::thing('event','ev1'); \
             RELATE $src->in_campaign->$dst SET created_at = time::now()",
        )
        .await
        .unwrap();

        let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
        assert!(
            result.contains("[event] Battle of Irongate"),
            "missing event: {result}"
        );
        assert!(
            result.contains("Year 312 → Year 313"),
            "missing dates: {result}"
        );
    }

    #[tokio::test]
    async fn fetch_entity_context_includes_entity_notes() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(
            "CREATE npc SET id='npc1', name='Seraphina', summary='archivist', \
             notes='She secretly guards the Sunstone beneath the Iron Tower.', \
             created_at=time::now(), updated_at=time::now(); \
             LET $src = type::thing('campaign','camp1'); \
             LET $dst = type::thing('npc','npc1'); \
             RELATE $src->in_campaign->$dst SET created_at = time::now()",
        )
        .await
        .unwrap();

        let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
        assert!(
            result.contains("Notes: She secretly guards the Sunstone"),
            "entity notes should appear in context: {result}"
        );
    }

    #[tokio::test]
    async fn fetch_entity_context_includes_session_notes() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(
            "CREATE session SET id='sess1', campaign=type::thing('campaign','camp1'), \
             session_number=4, title='Shadows of the Keep', date_played='2026-06-05', \
             notes='The party freed the prisoners and burned the granary.', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
        assert!(
            result.contains("[session 4] Shadows of the Keep"),
            "session line should appear in context: {result}"
        );
        assert!(
            result.contains("Notes: The party freed the prisoners"),
            "session notes should appear in context: {result}"
        );
    }

    #[tokio::test]
    async fn fetch_entity_context_event_empty_date_end_no_arrow() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(
            "CREATE event SET id='ev1', \
             name='Siege of Dawnwall', date_start='Year 400', date_end='', \
             summary=NULL, notes=NULL, is_ongoing=false, \
             created_at=time::now(), updated_at=time::now(); \
             LET $src = type::thing('campaign','camp1'); \
             LET $dst = type::thing('event','ev1'); \
             RELATE $src->in_campaign->$dst SET created_at = time::now()",
        )
        .await
        .unwrap();

        let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
        assert!(
            result.contains("[event] Siege of Dawnwall"),
            "missing event: {result}"
        );
        assert!(result.contains("Year 400"), "missing date_start: {result}");
        assert!(
            !result.contains("→"),
            "unexpected arrow when date_end is empty: {result}"
        );
    }

    /// Regression test for the entity-context KNN query bug: the collection
    /// branch built `ORDER BY embedding <|10|> $vec`, which SurrealDB rejects
    /// with "Missing order idiom `embedding` in statement selection" because the
    /// order idiom isn't in the projection. The KNN operator must live in WHERE
    /// (to activate the MTREE index) and ordering must be by
    /// `vector::distance::knn()`. Every other test passes `None` for the query
    /// embedding, so none of them ever reached this branch.
    #[tokio::test]
    async fn fetch_entity_context_knn_over_collection_executes() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        // 768-dim embeddings (matches the MTREE index dimension in the schema).
        let embedding: Vec<f32> = (0..768).map(|i| (i as f32) * 0.001).collect();
        let vec_str = embedding
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(",");

        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now(); \
             CREATE collection SET id='col1', name='Lore', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        // NPC linked to the collection (not the campaign) with an embedding so
        // the KNN branch has a row to rank.
        db.query(format!(
            "CREATE npc SET id='npc1', name='Seraphine', summary='oracle', notes=NULL, \
             embedding=[{vec_str}], embed_model='test', \
             created_at=time::now(), updated_at=time::now(); \
             LET $src = type::thing('collection','col1'); \
             LET $dst = type::thing('npc','npc1'); \
             RELATE $src->in_collection->$dst SET created_at = time::now()"
        ))
        .await
        .unwrap();

        // The buggy query failed to even parse; the assertion is that this
        // returns Ok and surfaces the collection entity.
        let result = fetch_entity_context(&db, "camp1", &["col1".to_string()], Some(&embedding))
            .await
            .expect("entity-context KNN query must be valid SurrealQL");
        assert!(
            result.contains("[npc] Seraphine"),
            "collection entity missing from KNN result: {result}"
        );
    }
}
