//! Manual lint pass (ADR-009 C2): pure-Rust detectors that surface data drift
//! as `lint_finding` rows for the Maintenance inbox. No LLM calls here —
//! contradiction detection is explicitly deferred.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;
use serde_json::json;
use surrealdb::sql::Thing;
use surrealdb::Connection;

use super::record_lint;
use crate::entity_service::{check_scope, EntityError};
use crate::naming;
use crate::wikilink::{query_all_entity_names, resolve_exact, EntityIdentity, WikilinkScope};

// Duplicated from `wikilink::mod` per the brief: widening that internal is
// not worth it for a one-line regex.
static WIKILINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\[\]]+)\]\]").expect("wikilink regex is valid"));

/// Project the `(id, name)` pairs a few detectors need out of the full
/// identity list. Kept separate from [`EntityIdentity`] itself because those
/// detectors (duplicates, staleness, scope) never look at aliases.
fn id_name_pairs(entities: &[EntityIdentity]) -> Vec<(String, String)> {
    entities
        .iter()
        .map(|e| (e.id.clone(), e.name.clone()))
        .collect()
}

/// Result of one lint pass.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LintSummary {
    /// Findings created by this run (dedup already applied).
    pub new_findings: usize,
    /// Total unresolved findings across all kinds after this run.
    pub unresolved_total: usize,
}

/// One unresolved finding (payload is kind-shaped; read-only Value is safe).
#[derive(Debug, Clone, serde::Serialize)]
pub struct LintFinding {
    /// Record id fragment (without the `lint_finding:` table prefix).
    pub id: String,
    /// One of `broken_wikilink`, `duplicate_entity`, `stale_article`,
    /// `scope_violation`.
    pub kind: String,
    /// Kind-shaped payload; see the detector that produced it.
    pub payload: serde_json::Value,
    /// ISO-ish timestamp string as rendered by SurrealDB's `Datetime`.
    pub created_at: String,
}

// NOTE: dedup checks below fetch matching rows and check emptiness rather
// than `SELECT count() ... GROUP ALL`. In this SurrealDB version, `count()`
// combined with `GROUP ALL` over an indexed field (`lint_finding.kind` has
// a `DEFINE INDEX`) silently ignores the `WHERE` clause and returns the
// table's total row count instead of the filtered count — verified via a
// two-kind fixture where `WHERE kind = 'x' GROUP ALL` returned the count of
// *all* kinds. Plain row selection is unaffected by this and is used
// throughout this module for that reason.

/// True when an unresolved finding of `kind` whose payload field `key`
/// equals `value` already exists (idempotent re-runs).
async fn finding_exists<C: Connection>(
    db: &surrealdb::Surreal<C>,
    kind: &str,
    key: &str,
    value: &str,
) -> Result<bool, String> {
    #[derive(Deserialize)]
    struct IdRow {
        // Existence check only; the id value itself is never read.
        #[allow(dead_code)]
        id: Thing,
    }
    let mut resp = db
        .query(format!(
            "SELECT id FROM lint_finding WHERE kind = $kind \
                 AND resolved_at = NONE AND payload.`{key}` = $val LIMIT 1"
        ))
        .bind(("kind", kind.to_owned()))
        .bind(("val", value.to_owned()))
        .await
        .map_err(|e| format!("Failed lint dedup check: {e}"))?;
    let rows: Vec<IdRow> = resp
        .take(0)
        .map_err(|e| format!("Failed lint dedup parse: {e}"))?;
    Ok(!rows.is_empty())
}

/// Same as [`finding_exists`] but requires both `key_a`/`value_a` AND
/// `key_b`/`value_b` to match (two-key dedup for `broken_wikilink` and
/// `duplicate_entity`).
async fn finding_exists_2<C: Connection>(
    db: &surrealdb::Surreal<C>,
    kind: &str,
    key_a: &str,
    value_a: &str,
    key_b: &str,
    value_b: &str,
) -> Result<bool, String> {
    #[derive(Deserialize)]
    struct IdRow {
        // Existence check only; the id value itself is never read.
        #[allow(dead_code)]
        id: Thing,
    }
    let mut resp = db
        .query(format!(
            "SELECT id FROM lint_finding WHERE kind = $kind \
                 AND resolved_at = NONE \
                 AND payload.`{key_a}` = $val_a AND payload.`{key_b}` = $val_b LIMIT 1"
        ))
        .bind(("kind", kind.to_owned()))
        .bind(("val_a", value_a.to_owned()))
        .bind(("val_b", value_b.to_owned()))
        .await
        .map_err(|e| format!("Failed lint dedup check: {e}"))?;
    let rows: Vec<IdRow> = resp
        .take(0)
        .map_err(|e| format!("Failed lint dedup parse: {e}"))?;
    Ok(!rows.is_empty())
}

/// Split a full record id like `"npc:abc123"` into `("npc", "abc123")`.
/// Backticks are stripped defensively — ids come from trusted internal
/// queries but interpolation into inline arrays warrants care.
fn split_full_id(full_id: &str) -> Option<(&str, &str)> {
    let pos = full_id.find(':')?;
    Some((&full_id[..pos], &full_id[pos + 1..]))
}

fn strip_backticks(id: &str) -> String {
    id.replace('`', "")
}

/// Group full entity ids `("table:id", name)` by table, producing a
/// `table -> [id, ...]` map with backtick-escaped ids ready for an inline
/// `IN [table:`id`, ...]` array.
fn group_ids_by_table(entities: &[(String, String)]) -> HashMap<&str, Vec<String>> {
    let mut by_table: HashMap<&str, Vec<String>> = HashMap::new();
    for (full_id, _name) in entities {
        if let Some((table, id)) = split_full_id(full_id) {
            by_table.entry(table).or_default().push(strip_backticks(id));
        }
    }
    by_table
}

/// Build an inline SurrealQL id array literal, e.g. `[npc:\`a\`, npc:\`b\`]`.
fn inline_id_array(table: &str, ids: &[String]) -> String {
    let items: Vec<String> = ids.iter().map(|id| format!("{table}:`{id}`")).collect();
    format!("[{}]", items.join(", "))
}

// ── Detector 1: broken wikilinks ────────────────────────────────────────────

async fn lint_broken_wikilinks<C: Connection>(
    db: &surrealdb::Surreal<C>,
    entities: &[EntityIdentity],
) -> Result<usize, String> {
    let pairs = id_name_pairs(entities);
    let by_table = group_ids_by_table(&pairs);
    let mut new_findings = 0;

    for (table, ids) in &by_table {
        if ids.is_empty() {
            continue;
        }
        let id_array = inline_id_array(table, ids);
        let query = format!("SELECT id, notes, codex_article FROM {table} WHERE id IN {id_array}");
        #[derive(Deserialize)]
        struct Row {
            id: Thing,
            notes: Option<String>,
            #[serde(default)]
            codex_article: Option<String>,
        }
        let mut resp = db
            .query(query)
            .await
            .map_err(|e| format!("Failed to fetch notes for lint: {e}"))?;
        let rows: Vec<Row> = resp
            .take(0)
            .map_err(|e| format!("Failed to parse notes for lint: {e}"))?;

        for row in rows {
            let full_id = format!("{}:{}", row.id.tb, row.id.id.to_raw());
            // Scan both fields — compiled articles carry their own
            // [[wikilinks]] independent of the source notes — but dedup on
            // (entity, link_text) so the same broken link found in both
            // yields a single finding.
            let texts = [row.notes, row.codex_article];
            let mut seen_links: Vec<String> = Vec::new();
            for text in texts.into_iter().flatten() {
                if text.is_empty() {
                    continue;
                }
                for cap in WIKILINK_RE.captures_iter(&text) {
                    let link_text = cap[1].trim().to_string();
                    if link_text.is_empty() {
                        continue;
                    }
                    let lower = link_text.to_lowercase();
                    if seen_links.iter().any(|l| l == &lower) {
                        continue;
                    }
                    // Route through the SAME resolver the fuzzy/exact tiers use
                    // (ADR-009 C2 fold-in): a link that resolves via an alias or
                    // a normalized-name match must never be reported broken —
                    // the linter and the resolver must never disagree about
                    // whether a link works.
                    if resolve_exact(&link_text, entities).is_some() {
                        continue;
                    }
                    seen_links.push(lower);
                    if finding_exists_2(
                        db,
                        "broken_wikilink",
                        "entity",
                        &full_id,
                        "link_text",
                        &link_text,
                    )
                    .await?
                    {
                        continue;
                    }
                    // A lower bar than auto-resolve on purpose: a SUGGESTION may
                    // be speculative because the GM adjudicates it, unlike tier
                    // 4's auto-resolve which must be certain enough to act on
                    // unattended.
                    let candidates = match naming::best_match(
                        &link_text,
                        &pairs,
                        naming::DEFAULT_THRESHOLD * 0.8,
                    ) {
                        naming::MatchOutcome::Unique { id, name, score } => {
                            vec![json!({ "id": id, "name": name, "similarity": score })]
                        }
                        naming::MatchOutcome::Ambiguous(cs) => cs
                            .iter()
                            .map(|c| json!({ "id": c.id, "name": c.name, "similarity": c.similarity }))
                            .collect(),
                        naming::MatchOutcome::None => vec![],
                    };
                    record_lint(
                        db,
                        "broken_wikilink",
                        json!({ "entity": full_id, "link_text": link_text, "candidates": candidates }),
                    )
                    .await?;
                    new_findings += 1;
                }
            }
        }
    }

    Ok(new_findings)
}

// ── Detector 2: duplicate entities ──────────────────────────────────────────

async fn lint_duplicates<C: Connection>(
    db: &surrealdb::Surreal<C>,
    entities: &[(String, String)],
) -> Result<usize, String> {
    let mut groups: HashMap<(String, String), Vec<String>> = HashMap::new();
    for (full_id, name) in entities {
        let Some((table, _id)) = split_full_id(full_id) else {
            continue;
        };
        let key = (table.to_string(), name.trim().to_lowercase());
        groups.entry(key).or_default().push(full_id.clone());
    }

    let mut new_findings = 0;
    for ids in groups.into_values() {
        if ids.len() < 2 {
            continue;
        }
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let mut pair = [ids[i].clone(), ids[j].clone()];
                pair.sort();
                let [a, b] = pair;
                if finding_exists_2(db, "duplicate_entity", "a", &a, "b", &b).await? {
                    continue;
                }
                record_lint(
                    db,
                    "duplicate_entity",
                    json!({ "a": a, "b": b, "similarity": 1.0 }),
                )
                .await?;
                new_findings += 1;
            }
        }
    }

    Ok(new_findings)
}

// ── Detector 3: stale / uncompiled articles ─────────────────────────────────

async fn lint_stale_articles<C: Connection>(
    db: &surrealdb::Surreal<C>,
    entities: &[(String, String)],
) -> Result<usize, String> {
    let by_table = group_ids_by_table(entities);
    let mut new_findings = 0;

    for (table, ids) in &by_table {
        if ids.is_empty() {
            continue;
        }
        let id_array = inline_id_array(table, ids);
        let query = format!(
            "SELECT id FROM {table} WHERE id IN {id_array} \
             AND (codex_stale != false OR codex_article = NONE)"
        );
        #[derive(Deserialize)]
        struct Row {
            id: Thing,
        }
        let mut resp = db
            .query(query)
            .await
            .map_err(|e| format!("Failed to fetch stale entities for lint: {e}"))?;
        let rows: Vec<Row> = resp
            .take(0)
            .map_err(|e| format!("Failed to parse stale entities for lint: {e}"))?;

        for row in rows {
            let full_id = format!("{}:{}", row.id.tb, row.id.id.to_raw());
            if finding_exists(db, "stale_article", "entity", &full_id).await? {
                continue;
            }
            record_lint(
                db,
                "stale_article",
                json!({ "entity": full_id, "reason": "stale or uncompiled" }),
            )
            .await?;
            new_findings += 1;
        }
    }

    Ok(new_findings)
}

// ── Detector 4: scope violations ────────────────────────────────────────────

async fn lint_scope_violations<C: Connection>(
    db: &surrealdb::Surreal<C>,
    entities: &[(String, String)],
) -> Result<usize, String> {
    let by_table = group_ids_by_table(entities);
    let mut combined: Vec<String> = Vec::new();
    for (table, ids) in &by_table {
        for id in ids {
            combined.push(format!("{table}:`{id}`"));
        }
    }
    if combined.is_empty() {
        return Ok(0);
    }
    let id_array = format!("[{}]", combined.join(", "));

    #[derive(Deserialize)]
    struct EdgeRow {
        id: Thing,
        #[serde(rename = "in")]
        in_: Thing,
        out: Thing,
    }
    let query =
        format!("SELECT id, in, out FROM relates_to WHERE in IN {id_array} OR out IN {id_array}");
    let mut resp = db
        .query(query)
        .await
        .map_err(|e| format!("Failed to fetch relates_to for lint: {e}"))?;
    let rows: Vec<EdgeRow> = resp
        .take(0)
        .map_err(|e| format!("Failed to parse relates_to for lint: {e}"))?;

    let mut new_findings = 0;
    for edge in rows {
        let from_table = edge.in_.tb.clone();
        let from_id = edge.in_.id.to_raw();
        let to_table = edge.out.tb.clone();
        let to_id = edge.out.id.to_raw();

        let result = check_scope(db, &from_table, &from_id, &to_table, &to_id).await;
        let (from, to) = match result {
            Ok(()) => continue,
            Err(EntityError::ScopeViolation { from, to }) => (from, to),
            Err(other) => return Err(other.to_string()),
        };

        let edge_id = format!("relates_to:{}", edge.id.id.to_raw());
        if finding_exists(db, "scope_violation", "edge", &edge_id).await? {
            continue;
        }
        record_lint(
            db,
            "scope_violation",
            json!({ "edge": edge_id, "from": from, "to": to }),
        )
        .await?;
        new_findings += 1;
    }

    Ok(new_findings)
}

// ── Detector 5: alias collisions ────────────────────────────────────────────

/// Two entities in the same resolution scope must never claim the same name
/// or alias, or tier-2 wikilink resolution stops being deterministic — the
/// same link would resolve to whichever entity happens to sort first. Group
/// every in-scope entity's name AND aliases by their normalized form; any
/// normalized key claimed by two different records gets one finding.
async fn lint_alias_collisions<C: Connection>(
    db: &surrealdb::Surreal<C>,
    entities: &[EntityIdentity],
) -> Result<usize, String> {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for entity in entities {
        let mut keys: Vec<String> = vec![naming::normalize(&entity.name)];
        keys.extend(entity.aliases.iter().map(|a| naming::normalize(a)));
        keys.sort_unstable();
        keys.dedup();
        for key in keys {
            if key.is_empty() {
                continue;
            }
            groups.entry(key).or_default().push(entity.id.clone());
        }
    }

    let mut new_findings = 0;
    for (alias, mut ids) in groups {
        ids.sort_unstable();
        ids.dedup();
        if ids.len() < 2 {
            continue;
        }
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let (a, b) = (ids[i].clone(), ids[j].clone());
                if finding_exists_2(db, "alias_collision", "a", &a, "b", &b).await? {
                    continue;
                }
                record_lint(
                    db,
                    "alias_collision",
                    json!({ "alias": alias, "a": a, "b": b }),
                )
                .await?;
                new_findings += 1;
            }
        }
    }

    Ok(new_findings)
}

// ── Pass entry points ────────────────────────────────────────────────────────

/// Run every detector over a campaign's full scope (own + subscribed).
pub async fn run_lint_campaign<C: Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
) -> Result<LintSummary, String> {
    let entities: Vec<EntityIdentity> =
        query_all_entity_names(db, &WikilinkScope::Campaign { campaign_id })
            .await
            .map_err(|e| e.to_string())?;
    run_detectors(db, &entities).await
}

/// Run every detector over a single collection's scope.
pub async fn run_lint_collection<C: Connection>(
    db: &surrealdb::Surreal<C>,
    collection_id: &str,
) -> Result<LintSummary, String> {
    let entities: Vec<EntityIdentity> =
        query_all_entity_names(db, &WikilinkScope::Collection { collection_id })
            .await
            .map_err(|e| e.to_string())?;
    run_detectors(db, &entities).await
}

async fn run_detectors<C: Connection>(
    db: &surrealdb::Surreal<C>,
    entities: &[EntityIdentity],
) -> Result<LintSummary, String> {
    let pairs = id_name_pairs(entities);
    let mut new_findings = 0;
    new_findings += lint_broken_wikilinks(db, entities).await?;
    new_findings += lint_duplicates(db, &pairs).await?;
    new_findings += lint_stale_articles(db, &pairs).await?;
    new_findings += lint_scope_violations(db, &pairs).await?;
    new_findings += lint_alias_collisions(db, entities).await?;
    let unresolved_total = unresolved_count(db).await?;
    Ok(LintSummary {
        new_findings,
        unresolved_total,
    })
}

async fn unresolved_count<C: Connection>(db: &surrealdb::Surreal<C>) -> Result<usize, String> {
    // See the note above `finding_exists`: avoid `count() ... GROUP ALL`
    // over the indexed `resolved_at` field; select ids and count them instead.
    #[derive(Deserialize)]
    struct IdRow {
        // Existence check only; the id value itself is never read.
        #[allow(dead_code)]
        id: Thing,
    }
    let mut resp = db
        .query("SELECT id FROM lint_finding WHERE resolved_at = NONE")
        .await
        .map_err(|e| format!("Failed to count findings: {e}"))?;
    let rows: Vec<IdRow> = resp
        .take(0)
        .map_err(|e| format!("Failed to parse finding count: {e}"))?;
    Ok(rows.len())
}

// ── List / resolve ───────────────────────────────────────────────────────────

/// All unresolved findings, newest first.
pub async fn list_lint_findings<C: Connection>(
    db: &surrealdb::Surreal<C>,
) -> Result<Vec<LintFinding>, String> {
    #[derive(Deserialize)]
    struct Row {
        id: Thing,
        kind: String,
        payload: serde_json::Value,
        created_at: surrealdb::sql::Datetime,
    }
    let mut resp = db
        .query(
            "SELECT id, kind, payload, created_at FROM lint_finding \
             WHERE resolved_at = NONE ORDER BY created_at DESC",
        )
        .await
        .map_err(|e| format!("Failed to list findings: {e}"))?;
    let rows: Vec<Row> = resp
        .take(0)
        .map_err(|e| format!("Failed to parse findings: {e}"))?;
    Ok(rows
        .into_iter()
        .map(|r| LintFinding {
            id: r.id.id.to_raw(),
            kind: r.kind,
            payload: r.payload,
            created_at: r.created_at.to_string(),
        })
        .collect())
}

/// Mark one finding resolved.
pub async fn resolve_lint_finding<C: Connection>(
    db: &surrealdb::Surreal<C>,
    finding_id: &str,
) -> Result<(), String> {
    db.query("UPDATE type::thing('lint_finding', $id) SET resolved_at = time::now()")
        .bind(("id", finding_id.to_owned()))
        .await
        .map_err(|e| format!("Failed to resolve finding: {e}"))?
        .check()
        .map_err(|e| format!("Failed to resolve finding: {e}"))?;
    Ok(())
}
