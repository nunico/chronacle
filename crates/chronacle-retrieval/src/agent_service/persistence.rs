//! Persisting chat messages, including assistant messages with parsed citations.

use surrealdb::Connection;

use super::citation::parse_citations;
use super::AgentError;

/// Insert a message record into the `message` table.
///
/// When `campaign_id` is `Some`, the message is bound to that campaign so
/// `get_chat_history` can filter per-campaign. `None` records a globally
/// scoped message (kept for the zero-campaign bootstrap window).
pub async fn persist_message<C>(
    db: &surrealdb::Surreal<C>,
    role: &str,
    content: &str,
    campaign_id: Option<&str>,
) -> Result<(), AgentError>
where
    C: Connection,
{
    let sql = match campaign_id {
        Some(_) => {
            "CREATE message SET
                role = $role,
                content = $content,
                citations = [],
                campaign = type::thing('campaign', $cid),
                created_at = time::now()"
        }
        None => {
            "CREATE message SET
                role = $role,
                content = $content,
                citations = [],
                created_at = time::now()"
        }
    };

    let mut q = db
        .query(sql)
        .bind(("role", role.to_owned()))
        .bind(("content", content.to_owned()));
    if let Some(cid) = campaign_id {
        q = q.bind(("cid", cid.to_owned()));
    }
    q.await.map_err(|e| AgentError::Db(e.to_string()))?;

    Ok(())
}

/// Persist an assistant message with parsed citations.
pub async fn persist_assistant_message<C>(
    db: &surrealdb::Surreal<C>,
    content: &str,
    campaign_id: Option<&str>,
) -> Result<(), AgentError>
where
    C: Connection,
{
    let citations = parse_citations(content);

    if citations.is_empty() {
        return persist_message(db, "assistant", content, campaign_id).await;
    }

    // Build citations as SurrealQL inline objects (bind params lose field names
    // with serde_json::Value for array<object> types)
    let cit_parts: Vec<String> = citations
        .iter()
        .map(|c| {
            let name = c.source_name.replace('\'', "''");
            let excerpt = c.text_excerpt.replace('\'', "''");
            match c.page {
                Some(p) => {
                    format!("{{ source_name: '{name}', page: {p}, text_excerpt: '{excerpt}' }}")
                }
                None => format!("{{ source_name: '{name}', text_excerpt: '{excerpt}' }}"),
            }
        })
        .collect();
    let cit_surql = cit_parts.join(", ");

    let campaign_assign = if campaign_id.is_some() {
        ", campaign = type::thing('campaign', $cid)"
    } else {
        ""
    };

    let sql = format!(
        "CREATE message SET \
         role = 'assistant', \
         content = $content, \
         citations = [{cit_surql}]\
         {campaign_assign}, \
         created_at = time::now()"
    );

    let mut q = db.query(sql).bind(("content", content.to_owned()));
    if let Some(cid) = campaign_id {
        q = q.bind(("cid", cid.to_owned()));
    }
    q.await.map_err(|e| AgentError::Db(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_persist_and_retrieve_messages() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();

        db.query(
            "DEFINE TABLE message SCHEMAFULL;
             DEFINE FIELD role ON message TYPE string;
             DEFINE FIELD content ON message TYPE string;
             DEFINE FIELD citations ON message TYPE array<object>;
             DEFINE FIELD created_at ON message TYPE datetime;",
        )
        .await
        .unwrap();

        persist_message(&db, "user", "question", None)
            .await
            .unwrap();
        persist_assistant_message(&db, "response with [Source: \"PHB\", p.72].", None)
            .await
            .unwrap();

        let mut response = db
            .query("SELECT count() FROM message GROUP ALL")
            .await
            .unwrap();
        #[derive(serde::Deserialize)]
        struct Count {
            count: i64,
        }
        let counts: Vec<Count> = response.take(0).unwrap();
        assert_eq!(counts[0].count, 2);

        let citations = parse_citations("response with [Source: \"PHB\", p.72].");
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].source_name, "PHB");
        assert_eq!(citations[0].page, Some(72));

        let mut response = db
            .query("SELECT role, content FROM message WHERE role = $role")
            .bind(("role", "assistant"))
            .await
            .unwrap();
        #[derive(serde::Deserialize)]
        #[expect(dead_code)]
        struct Msg {
            role: String,
            content: String,
        }
        let msgs: Vec<Msg> = response.take(0).unwrap();
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].content.contains("PHB"));
    }

    #[tokio::test]
    async fn test_chat_history_empty_when_no_messages() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();

        db.query(
            "DEFINE TABLE message SCHEMAFULL;
             DEFINE FIELD role ON message TYPE string;
             DEFINE FIELD content ON message TYPE string;
             DEFINE FIELD citations ON message TYPE array<object>;
             DEFINE FIELD created_at ON message TYPE datetime;",
        )
        .await
        .unwrap();

        let mut response = db
            .query("SELECT role, content FROM message LIMIT 10")
            .await
            .unwrap();

        #[derive(serde::Deserialize)]
        #[expect(dead_code)]
        struct Row {
            role: String,
            content: String,
        }

        let rows: Vec<Row> = response.take(0).unwrap();
        assert!(rows.is_empty());
    }

    /// Regression for bug #3: messages must be bound to the active campaign so
    /// `get_chat_history` can filter per-campaign instead of returning every row
    /// when the user switches campaigns.
    #[tokio::test]
    async fn persist_message_binds_campaign_record_link() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        chronacle_db::run_migrations(&db).await.unwrap();

        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        persist_message(&db, "user", "scoped to camp1", Some("camp1"))
            .await
            .unwrap();
        persist_assistant_message(&db, "reply [Source: \"PHB\", p.72].", Some("camp1"))
            .await
            .unwrap();
        // A separate "global" message that must not leak into the camp1 filter.
        persist_message(&db, "user", "unscoped", None)
            .await
            .unwrap();

        #[derive(serde::Deserialize)]
        struct Row {
            content: String,
        }
        let mut resp = db
            .query(
                "SELECT content, created_at FROM message \
                 WHERE campaign = type::thing('campaign', 'camp1') \
                 ORDER BY created_at ASC",
            )
            .await
            .unwrap();
        let rows: Vec<Row> = resp.take(0).unwrap();
        assert_eq!(rows.len(), 2, "exactly the two camp1-scoped messages");
        assert_eq!(rows[0].content, "scoped to camp1");
        assert!(rows[1].content.starts_with("reply"));
    }

    /// Regression for bug #5: history was lost on Oracle re-mount because
    /// `get_chat_history` filters using a literal `campaign:`<id>`` record link
    /// while persistence wrote `type::thing('campaign', $cid)`. The two MUST
    /// produce the same record id so the filter matches, especially for
    /// real-world hex-only campaign IDs (UUIDs with hyphens stripped).
    #[tokio::test]
    async fn chat_history_literal_filter_matches_persisted_messages() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        chronacle_db::run_migrations(&db).await.unwrap();

        // Use the same id shape `campaign_service::create` produces — a UUID
        // with hyphens stripped, so the WHERE literal does not need backticks
        // around special characters.
        let cid = "d5a80195396844cb8b46270830df952f";
        db.query(format!(
            "CREATE campaign SET id='{cid}', name='T', system='5e', \
             created_at=time::now(), updated_at=time::now()"
        ))
        .await
        .unwrap();

        persist_message(&db, "user", "first", Some(cid))
            .await
            .unwrap();
        persist_assistant_message(&db, "reply", Some(cid))
            .await
            .unwrap();

        // Mirror the exact SQL `commands::get_chat_history` issues.
        let safe_id = cid.replace('`', "``");
        let sql = format!(
            "SELECT role, content, created_at FROM message \
             WHERE campaign = campaign:`{safe_id}` ORDER BY created_at ASC"
        );
        #[derive(serde::Deserialize)]
        struct Row {
            role: String,
            content: String,
        }
        let mut resp = db.query(sql).await.unwrap();
        let rows: Vec<Row> = resp.take(0).unwrap();
        assert_eq!(
            rows.len(),
            2,
            "literal-record-link filter must match `type::thing`-written messages"
        );
        assert_eq!(rows[0].role, "user");
        assert_eq!(rows[0].content, "first");
        assert_eq!(rows[1].role, "assistant");
    }
}
