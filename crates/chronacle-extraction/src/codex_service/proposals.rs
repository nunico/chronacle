//! Write-back review queue (ADR-009 C1): LLM producers distill chat answers
//! and session notes into `codex_proposal` rows; nothing mutates the compiled
//! layer until a proposal is explicitly accepted.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use surrealdb::Connection;

use super::prompts::{build_chat_distill_prompt, build_session_distill_prompt};
use super::CodexError;
use crate::extraction_service::llm_complete;
use crate::wikilink::{query_all_entity_names, WikilinkScope};
use chronacle_core::llm::{ChatMessage, LlmProvider};

/// Cap on proposals persisted per distillation run (cost/noise control).
pub const MAX_PROPOSALS_PER_DISTILL: usize = 8;

const SYSTEM_PROMPT: &str =
    "You are a careful TTRPG knowledge-base maintainer. Propose only what the text supports.";

const PROPOSAL_KINDS: [&str; 5] = [
    "entity_article_update",
    "entity_notes_update",
    "rule_entry_update",
    "new_entity",
    "new_rule_entry",
];

/// The payload persisted on a proposal (FLEXIBLE object — plain struct bind).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalPayload {
    pub proposed_text: String,
    pub rationale: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub entity_kind: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
}

/// Frontend-facing proposal DTO, enriched with the target's display name and
/// the current text of the field the proposal would change (for diff preview).
#[derive(Debug, Clone, Serialize)]
pub struct CodexProposal {
    pub id: String,
    pub kind: String,
    pub target: Option<String>,
    pub target_name: Option<String>,
    pub current_text: Option<String>,
    pub payload: ProposalPayload,
    pub origin_kind: String,
    pub status: String,
    pub created_at: String,
}

/// Pending work counts for the Maintenance badge.
#[derive(Debug, Clone, Serialize)]
pub struct MaintenanceCounts {
    pub pending_proposals: usize,
    pub unresolved_findings: usize,
}

// ── Tolerant JSON parsing (same discipline as rules.rs) ─────────────────────

fn strip_code_fences(raw: &str) -> &str {
    let trimmed = raw.trim();
    if let Some(s) = trimmed.strip_prefix("```json") {
        s.trim_end_matches("```").trim()
    } else if let Some(s) = trimmed.strip_prefix("```") {
        s.trim_end_matches("```").trim()
    } else {
        trimmed
    }
}

#[derive(Debug, Default, Deserialize)]
struct DistillResponse {
    #[serde(default)]
    proposals: Vec<DraftProposal>,
    #[serde(default)]
    mentioned: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DraftProposal {
    kind: String,
    #[serde(default)]
    target_name: Option<String>,
    #[serde(default)]
    entity_kind: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    proposed_text: String,
    #[serde(default)]
    rationale: String,
}

fn parse_distill_response(raw: &str) -> DistillResponse {
    serde_json::from_str(strip_code_fences(raw)).unwrap_or_else(|e| {
        eprintln!("codex: proposal JSON parse failed ({e}), returning empty result");
        DistillResponse::default()
    })
}

/// The campaign's owned collection (ADR-010 auto-owned notes collection).
async fn owned_collection_id<C: Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
) -> Result<Option<String>, CodexError> {
    #[derive(Deserialize)]
    struct Row {
        id: Thing,
    }
    let mut resp = db
        .query("SELECT id FROM collection WHERE owner_campaign = type::thing('campaign', $cid) LIMIT 1")
        .bind(("cid", campaign_id.to_owned()))
        .await
        .map_err(|e| CodexError::Db(e.to_string()))?;
    let rows: Vec<Row> = resp.take(0).map_err(|e| CodexError::Db(e.to_string()))?;
    Ok(rows.into_iter().next().map(|r| r.id.id.to_raw()))
}

/// The collection an entity lives in (via `in_collection`), if any.
async fn entity_collection_id<C: Connection>(
    db: &surrealdb::Surreal<C>,
    full_id: &str, // "table:id"
) -> Result<Option<String>, CodexError> {
    let (table, id) = full_id.split_once(':').unwrap_or((full_id, ""));
    let mut resp = db
        .query("SELECT VALUE in FROM in_collection WHERE out = type::thing($t, $i) LIMIT 1")
        .bind(("t", table.to_owned()))
        .bind(("i", id.to_owned()))
        .await
        .map_err(|e| CodexError::Db(e.to_string()))?;
    let rows: Vec<Thing> = resp.take(0).map_err(|e| CodexError::Db(e.to_string()))?;
    Ok(rows.into_iter().next().map(|t| t.id.to_raw()))
}

/// Persist one resolved proposal. `target` is a full "table:id" or None.
#[allow(clippy::too_many_arguments)] // mirrors the shape of the fields being persisted
async fn create_proposal<C: Connection>(
    db: &surrealdb::Surreal<C>,
    kind: &str,
    target: Option<&str>,
    collection_id: &str,
    campaign_id: Option<&str>,
    payload: &ProposalPayload,
    origin_kind: &str,
    origin_ref: Option<(&str, String)>, // e.g. ("session", "<id>") or ("message", "<id>")
) -> Result<(), CodexError> {
    let target_expr = match target {
        Some(t) => {
            let (table, id) = t.split_once(':').unwrap_or((t, ""));
            format!(
                "type::thing('{}', '{}')",
                table.replace('\'', ""),
                id.replace('\'', "")
            )
        }
        None => "NONE".to_string(),
    };
    let campaign_expr = match campaign_id {
        Some(_) => "type::thing('campaign', $cam)",
        None => "NONE",
    };
    let origin_expr = match &origin_ref {
        Some((key, _)) => format!("{{ kind: $okind, {key}: $oref }}"),
        None => "{ kind: $okind }".to_string(),
    };
    let sql = format!(
        "CREATE codex_proposal SET kind = $kind, target = {target_expr}, \
             collection = type::thing('collection', $col), campaign = {campaign_expr}, \
             payload = $payload, origin = {origin_expr}, status = 'pending'"
    );
    let mut q = db
        .query(sql)
        .bind(("kind", kind.to_owned()))
        .bind(("col", collection_id.to_owned()))
        .bind(("payload", payload.clone()))
        .bind(("okind", origin_kind.to_owned()));
    if let Some(cam) = campaign_id {
        q = q.bind(("cam", cam.to_owned()));
    }
    if let Some((_, oref)) = origin_ref {
        q = q.bind(("oref", oref));
    }
    q.await
        .map_err(|e| CodexError::Db(e.to_string()))?
        .check()
        .map_err(|e| CodexError::Db(e.to_string()))?;
    Ok(())
}

/// Name → "table:id" list of the campaign's in-scope entities.
async fn known_entities<C: Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
) -> Result<Vec<(String, String)>, CodexError> {
    query_all_entity_names(db, &WikilinkScope::Campaign { campaign_id })
        .await
        .map_err(|e| CodexError::Db(e.to_string()))
}

fn resolve_target<'a>(
    known: &'a [(String, String)], // (full_id, name)
    name: &str,
) -> Option<&'a str> {
    let needle = name.trim().to_lowercase();
    known
        .iter()
        .find(|(_, n)| n.trim().to_lowercase() == needle)
        .map(|(id, _)| id.as_str())
}

async fn persist_drafts<C: Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
    drafts: Vec<DraftProposal>,
    origin_kind: &str,
    origin_ref: Option<(&str, String)>,
) -> Result<usize, CodexError> {
    let known = known_entities(db, campaign_id).await?;
    let owned = owned_collection_id(db, campaign_id).await?;
    let mut created = 0usize;
    for d in drafts {
        if created >= MAX_PROPOSALS_PER_DISTILL {
            break;
        }
        if !PROPOSAL_KINDS.contains(&d.kind.as_str()) || d.proposed_text.trim().is_empty() {
            continue;
        }
        let is_new = d.kind.starts_with("new_");
        let target = if is_new {
            None
        } else {
            let Some(n) = &d.target_name else {
                eprintln!(
                    "codex: skipping '{}' proposal — missing target_name",
                    d.kind
                );
                continue;
            };
            match resolve_target(&known, n) {
                Some(t) => Some(t.to_string()),
                None => {
                    eprintln!("codex: skipping proposal for unknown target '{n}'");
                    continue;
                }
            }
        };
        // Collection: the target's own collection when it has one; otherwise
        // the campaign's owned collection. No collection at all ⇒ skip (the
        // schema requires one).
        let collection = match &target {
            Some(t) => entity_collection_id(db, t).await?.or_else(|| owned.clone()),
            None => owned.clone(),
        };
        let Some(collection) = collection else {
            eprintln!("codex: skipping proposal — no collection resolvable");
            continue;
        };
        let payload = ProposalPayload {
            proposed_text: d.proposed_text.clone(),
            rationale: d.rationale.clone(),
            name: d.target_name.clone(),
            entity_kind: d.entity_kind.clone(),
            category: d.category.clone(),
        };
        create_proposal(
            db,
            &d.kind,
            target.as_deref(),
            &collection,
            Some(campaign_id),
            &payload,
            origin_kind,
            origin_ref.clone(),
        )
        .await?;
        created += 1;
    }
    Ok(created)
}

/// Distill a cited chat answer into pending proposals ("Save to Codex").
/// Origin links the newest assistant `message` row with this exact content
/// when one exists.
pub async fn distill_chat_answer<C: Connection>(
    db: &surrealdb::Surreal<C>,
    llm: &Arc<dyn LlmProvider>,
    campaign_id: &str,
    answer: &str,
) -> Result<usize, CodexError> {
    let known = known_entities(db, campaign_id).await?;
    let known_block = known
        .iter()
        .map(|(id, n)| format!("- {n} — {}", id.split(':').next().unwrap_or("?")))
        .collect::<Vec<_>>()
        .join("\n");
    let prompt = build_chat_distill_prompt(answer, &known_block);
    let messages = vec![ChatMessage {
        role: "user".to_string(),
        content: prompt,
    }];
    let raw = llm_complete(llm.as_ref(), SYSTEM_PROMPT, &messages)
        .await
        .map_err(|e| CodexError::Llm(e.to_string()))?;
    let parsed = parse_distill_response(&raw);

    // Best-effort origin: the persisted assistant message with this content.
    #[derive(Deserialize)]
    struct MsgRow {
        id: Thing,
    }
    let mut resp = db
        .query(
            "SELECT id, created_at FROM message WHERE role = 'assistant' AND content = $c \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(("c", answer.to_owned()))
        .await
        .map_err(|e| CodexError::Db(e.to_string()))?;
    let msg: Vec<MsgRow> = resp.take(0).map_err(|e| CodexError::Db(e.to_string()))?;
    let origin_ref = msg
        .into_iter()
        .next()
        .map(|m| ("message", m.id.id.to_raw()));

    persist_drafts(db, campaign_id, parsed.proposals, "chat", origin_ref).await
}

/// Distill saved session notes: create proposals and mark every mentioned
/// known entity `codex_stale`. Re-saving the same session first clears its
/// previous *pending* proposals so the queue never accumulates duplicates.
pub async fn distill_session_notes<C: Connection>(
    db: &surrealdb::Surreal<C>,
    llm: &Arc<dyn LlmProvider>,
    session_id: &str,
) -> Result<usize, CodexError> {
    #[derive(Deserialize)]
    struct SessionRow {
        campaign: Option<Thing>,
        notes: String,
    }
    let mut resp = db
        .query("SELECT campaign, notes FROM type::thing('session', $id)")
        .bind(("id", session_id.to_owned()))
        .await
        .map_err(|e| CodexError::Db(e.to_string()))?;
    let rows: Vec<SessionRow> = resp.take(0).map_err(|e| CodexError::Db(e.to_string()))?;
    let session = rows
        .into_iter()
        .next()
        .ok_or_else(|| CodexError::Db(format!("session {session_id} not found")))?;
    let Some(campaign) = session.campaign else {
        return Ok(0);
    };
    let campaign_id = campaign.id.to_raw();

    // Replace this session's previous pending proposals (idempotent re-save).
    // Runs even when notes are now empty, so clearing notes purges stale
    // pending proposals instead of leaving them in the queue forever.
    db.query(
        "DELETE codex_proposal WHERE status = 'pending' \
             AND origin.session = $sid AND origin.kind = 'session'",
    )
    .bind(("sid", session_id.to_owned()))
    .await
    .map_err(|e| CodexError::Db(e.to_string()))?
    .check()
    .map_err(|e| CodexError::Db(e.to_string()))?;

    if session.notes.trim().is_empty() {
        return Ok(0);
    }

    let known = known_entities(db, &campaign_id).await?;
    let known_block = known
        .iter()
        .map(|(id, n)| format!("- {n} — {}", id.split(':').next().unwrap_or("?")))
        .collect::<Vec<_>>()
        .join("\n");
    let prompt = build_session_distill_prompt(&session.notes, &known_block);
    let messages = vec![ChatMessage {
        role: "user".to_string(),
        content: prompt,
    }];
    let raw = llm_complete(llm.as_ref(), SYSTEM_PROMPT, &messages)
        .await
        .map_err(|e| CodexError::Llm(e.to_string()))?;
    let parsed = parse_distill_response(&raw);

    // Mark mentioned known entities stale.
    for name in &parsed.mentioned {
        if let Some(full) = resolve_target(&known, name) {
            let (table, id) = full.split_once(':').unwrap_or((full, ""));
            if let Err(e) = super::mark_entity_stale(db, table, id).await {
                eprintln!("codex: stale-mark failed for {full}: {e}");
            }
        }
    }

    persist_drafts(
        db,
        &campaign_id,
        parsed.proposals,
        "session",
        Some(("session", session_id.to_string())),
    )
    .await
}

#[derive(Debug, Deserialize)]
struct ProposalRow {
    id: Thing,
    kind: String,
    target: Option<Thing>,
    payload: ProposalPayload,
    origin: serde_json::Value, // read-only: FLEXIBLE reads into Value are safe
    status: String,
    created_at: surrealdb::sql::Datetime,
}

/// List proposals, newest first, optionally filtered by status. Each row is
/// enriched with the target's display name and the current text of the field
/// the proposal would change (for the diff preview).
pub async fn list_proposals<C: Connection>(
    db: &surrealdb::Surreal<C>,
    status: Option<&str>,
) -> Result<Vec<CodexProposal>, String> {
    let sql = match status {
        Some(_) => {
            "SELECT id, kind, target, payload, origin, status, created_at \
                    FROM codex_proposal WHERE status = $status ORDER BY created_at DESC"
        }
        None => {
            "SELECT id, kind, target, payload, origin, status, created_at \
                 FROM codex_proposal ORDER BY created_at DESC"
        }
    };
    let mut q = db.query(sql);
    if let Some(s) = status {
        q = q.bind(("status", s.to_owned()));
    }
    let mut resp = q
        .await
        .map_err(|e| format!("Failed to list proposals: {e}"))?;
    let rows: Vec<ProposalRow> = resp
        .take(0)
        .map_err(|e| format!("Failed to parse proposals: {e}"))?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let (target, target_name, current_text) = match &r.target {
            Some(t) => {
                let table = t.tb.clone();
                let id = t.id.to_raw();
                let field = match r.kind.as_str() {
                    "entity_notes_update" => "notes",
                    "rule_entry_update" => "body",
                    _ => "codex_article",
                };
                #[derive(Deserialize)]
                struct Enrich {
                    name: Option<String>,
                    current: Option<String>,
                }
                let mut er = db
                    .query(format!(
                        "SELECT name, {field} AS current FROM type::thing($t, $i)"
                    ))
                    .bind(("t", table.clone()))
                    .bind(("i", id.clone()))
                    .await
                    .map_err(|e| format!("Failed to enrich proposal: {e}"))?;
                let e: Option<Enrich> = er
                    .take(0)
                    .map_err(|e| format!("Failed to parse enrichment: {e}"))?;
                let e = e.unwrap_or(Enrich {
                    name: None,
                    current: None,
                });
                (Some(format!("{table}:{id}")), e.name, e.current)
            }
            None => (None, r.payload.name.clone(), None),
        };
        out.push(CodexProposal {
            id: r.id.id.to_raw(),
            kind: r.kind,
            target,
            target_name,
            current_text,
            payload: r.payload,
            origin_kind: r.origin["kind"].as_str().unwrap_or("manual").to_string(),
            status: r.status,
            created_at: r.created_at.to_string(),
        });
    }
    Ok(out)
}

/// Apply an accepted proposal to its target, append provenance, re-embed, and
/// resolve the row. The ONLY path by which machine text reaches the user-owned
/// `notes` field is the `entity_notes_update` arm below.
pub async fn accept_proposal<C: Connection>(
    db: &surrealdb::Surreal<C>,
    embed: &Arc<dyn chronacle_core::embedding::EmbeddingProvider>,
    proposal_id: &str,
) -> Result<(), String> {
    let mut resp = db
        .query(
            "SELECT id, kind, target, payload, origin, status, created_at \
             FROM type::thing('codex_proposal', $id)",
        )
        .bind(("id", proposal_id.to_owned()))
        .await
        .map_err(|e| format!("Failed to load proposal: {e}"))?;
    let rows: Vec<ProposalRow> = resp
        .take(0)
        .map_err(|e| format!("Failed to parse proposal: {e}"))?;
    let row = rows
        .into_iter()
        .next()
        .ok_or_else(|| format!("proposal {proposal_id} not found"))?;
    if row.status != "pending" {
        return Err(format!("proposal {proposal_id} is already {}", row.status));
    }
    let text = row.payload.proposed_text.clone();

    match row.kind.as_str() {
        "entity_article_update" => {
            let t = row.target.as_ref().ok_or("article update needs a target")?;
            db.query(
                "UPDATE type::thing($t, $i) SET codex_article = $text, \
                     codex_compiled_at = time::now(), codex_stale = false, \
                     codex_sources = array::append(codex_sources, \
                         { kind: 'proposal', proposal: type::thing('codex_proposal', $pid) })",
            )
            .bind(("t", t.tb.clone()))
            .bind(("i", t.id.to_raw()))
            .bind(("text", text))
            .bind(("pid", proposal_id.to_owned()))
            .await
            .map_err(|e| format!("Failed to apply article update: {e}"))?
            .check()
            .map_err(|e| format!("Failed to apply article update: {e}"))?;
            reembed_entity(db, embed, &t.tb, &t.id.to_raw()).await?;
        }
        "entity_notes_update" => {
            let t = row.target.as_ref().ok_or("notes update needs a target")?;
            db.query(
                "UPDATE type::thing($t, $i) SET notes = $text, codex_stale = true, \
                     updated_at = time::now()",
            )
            .bind(("t", t.tb.clone()))
            .bind(("i", t.id.to_raw()))
            .bind(("text", text))
            .await
            .map_err(|e| format!("Failed to apply notes update: {e}"))?
            .check()
            .map_err(|e| format!("Failed to apply notes update: {e}"))?;
            reembed_entity(db, embed, &t.tb, &t.id.to_raw()).await?;
        }
        "rule_entry_update" => {
            let t = row.target.as_ref().ok_or("rule update needs a target")?;
            db.query(
                "UPDATE type::thing('rule_entry', $i) SET body = $text, \
                     compiled_at = time::now(), stale = false, \
                     sources = array::append(sources, \
                         { kind: 'proposal', proposal: type::thing('codex_proposal', $pid) })",
            )
            .bind(("i", t.id.to_raw()))
            .bind(("text", text))
            .bind(("pid", proposal_id.to_owned()))
            .await
            .map_err(|e| format!("Failed to apply rule update: {e}"))?
            .check()
            .map_err(|e| format!("Failed to apply rule update: {e}"))?;
            reembed_rule(db, embed, &t.id.to_raw()).await?;
        }
        "new_entity" => {
            let name = row.payload.name.clone().ok_or("new_entity needs a name")?;
            let kind_str = row
                .payload
                .entity_kind
                .clone()
                .unwrap_or_else(|| "misc".into());
            let kind = crate::entity_service::EntityKind::from_table(&kind_str)
                .unwrap_or(crate::entity_service::EntityKind::Misc);
            let col = proposal_collection(db, proposal_id).await?;
            let input = crate::entity_service::EntityInput {
                name,
                summary: Some(row.payload.proposed_text.clone()),
                ..Default::default()
            };
            crate::entity_service::create(db, None, Some(&col), kind, input)
                .await
                .map_err(|e| format!("Failed to create entity: {e}"))?;
        }
        "new_rule_entry" => {
            let name = row
                .payload
                .name
                .clone()
                .ok_or("new_rule_entry needs a name")?;
            let category = row
                .payload
                .category
                .clone()
                .filter(|c| super::RULE_CATEGORIES.contains(&c.as_str()))
                .unwrap_or_else(|| "entry".into());
            let col = proposal_collection(db, proposal_id).await?;
            let mut resp = db
                .query(
                    "CREATE rule_entry SET collection = type::thing('collection', $cid), \
                         name = $name, category = $category, body = $body, \
                         compiled_at = time::now(), stale = false, \
                         sources = [{ kind: 'proposal', proposal: type::thing('codex_proposal', $pid) }] \
                         RETURN VALUE id",
                )
                .bind(("cid", col))
                .bind(("name", name))
                .bind(("category", category))
                .bind(("body", row.payload.proposed_text.clone()))
                .bind(("pid", proposal_id.to_owned()))
                .await
                .map_err(|e| format!("Failed to create rule entry: {e}"))?;
            let ids: Vec<Thing> = resp
                .take(0)
                .map_err(|e| format!("Failed to parse created rule entry: {e}"))?;
            if let Some(id) = ids.into_iter().next() {
                reembed_rule(db, embed, &id.id.to_raw()).await?;
            }
        }
        other => return Err(format!("unknown proposal kind '{other}'")),
    }

    db.query(
        "UPDATE type::thing('codex_proposal', $id) SET status = 'accepted', \
             resolved_at = time::now()",
    )
    .bind(("id", proposal_id.to_owned()))
    .await
    .map_err(|e| format!("Failed to resolve proposal: {e}"))?;
    Ok(())
}

/// Read back a proposal's required collection id (bare id).
async fn proposal_collection<C: Connection>(
    db: &surrealdb::Surreal<C>,
    proposal_id: &str,
) -> Result<String, String> {
    #[derive(Deserialize)]
    struct ColRow {
        collection: Thing,
    }
    let mut cr = db
        .query("SELECT collection FROM type::thing('codex_proposal', $id)")
        .bind(("id", proposal_id.to_owned()))
        .await
        .map_err(|e| format!("Failed to read proposal collection: {e}"))?;
    let col: Option<ColRow> = cr
        .take(0)
        .map_err(|e| format!("Failed to parse proposal collection: {e}"))?;
    Ok(col
        .ok_or("proposal has no collection")?
        .collection
        .id
        .to_raw())
}

/// Resolve a proposal without applying it.
pub async fn reject_proposal<C: Connection>(
    db: &surrealdb::Surreal<C>,
    proposal_id: &str,
) -> Result<(), String> {
    db.query(
        "UPDATE type::thing('codex_proposal', $id) SET status = 'rejected', \
             resolved_at = time::now()",
    )
    .bind(("id", proposal_id.to_owned()))
    .await
    .map_err(|e| format!("Failed to reject proposal: {e}"))?;
    Ok(())
}

/// Pending proposals + unresolved lint findings (Maintenance badge).
pub async fn maintenance_counts<C: Connection>(
    db: &surrealdb::Surreal<C>,
) -> Result<MaintenanceCounts, String> {
    #[derive(Deserialize)]
    struct Counts {
        proposals: usize,
        findings: usize,
    }
    let mut resp = db
        .query(
            "RETURN { proposals: array::len((SELECT VALUE id FROM codex_proposal WHERE status = 'pending')), \
                      findings: array::len((SELECT VALUE id FROM lint_finding WHERE resolved_at = NONE)) };",
        )
        .await
        .map_err(|e| format!("Failed to count maintenance items: {e}"))?;
    let row: Option<Counts> = resp
        .take(0)
        .map_err(|e| format!("Failed to parse maintenance counts: {e}"))?;
    let row = row.ok_or("maintenance count query returned nothing")?;
    Ok(MaintenanceCounts {
        pending_proposals: row.proposals,
        unresolved_findings: row.findings,
    })
}

/// Re-embed one entity (name + summary + notes + article) after an accept.
async fn reembed_entity<C: Connection>(
    db: &surrealdb::Surreal<C>,
    embed: &Arc<dyn chronacle_core::embedding::EmbeddingProvider>,
    table: &str,
    id: &str,
) -> Result<(), String> {
    let kind = crate::entity_service::EntityKind::from_table(table).map_err(|e| e.to_string())?;
    let node = crate::entity_service::get_by_id(db, id, kind)
        .await
        .map_err(|e| e.to_string())?;
    super::compile::embed_entity_with_article(db, embed, &node)
        .await
        .map_err(|e| e.to_string())
}

/// Re-embed one rule entry after an accept.
async fn reembed_rule<C: Connection>(
    db: &surrealdb::Surreal<C>,
    embed: &Arc<dyn chronacle_core::embedding::EmbeddingProvider>,
    id: &str,
) -> Result<(), String> {
    #[derive(Deserialize)]
    struct Row {
        name: String,
        category: String,
        body: String,
    }
    let mut resp = db
        .query("SELECT name, category, body FROM type::thing('rule_entry', $id)")
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| format!("Failed to load rule entry: {e}"))?;
    let row: Option<Row> = resp
        .take(0)
        .map_err(|e| format!("Failed to parse rule entry: {e}"))?;
    let Some(row) = row else {
        return Ok(());
    };
    super::rules::embed_rule_entry(db, embed, id, &row.name, &row.category, &row.body)
        .await
        .map_err(|e| e.to_string())
}
