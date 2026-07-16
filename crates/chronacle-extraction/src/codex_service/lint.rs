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

/// Order-independent pair, sorted so `(a, b)` and `(b, a)` always dedup to
/// the same key (used for `duplicate_entity` finding identity).
fn sorted_pair(a: String, b: String) -> (String, String) {
    let mut pair = [a, b];
    pair.sort();
    let [a, b] = pair;
    (a, b)
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

/// Detect duplicate entities WITHIN THE SAME TABLE in two stages, both
/// scoped to the `entities` the caller already resolved (own + subscribed
/// collections — see `run_lint_campaign`/`run_lint_collection`).
///
/// Stage 1: exact grouping on the shared [`naming::normalize`] key — the
/// same engine wikilink resolution uses, so the two detectors can never
/// disagree. Catches "The Free League" / "Free League" with no scoring,
/// similarity 1.0.
///
/// Stage 2: for pairs stage 1 didn't already report, score with
/// [`naming::similarity`] and report only pairs at or above
/// [`naming::DEFAULT_THRESHOLD`]. A false duplicate proposes a MERGE — data
/// loss if accepted — so a missed match (which just stays a duplicate) is
/// always preferred over a false one.
///
/// Never compares across tables: a faction and a similarly-named tavern
/// (location) are different kinds of thing and must never pair regardless
/// of string similarity.
async fn lint_duplicates<C: Connection>(
    db: &surrealdb::Surreal<C>,
    entities: &[(String, String)],
) -> Result<usize, String> {
    // Group (table, full_id, name) by table first — every subsequent
    // comparison stays within one table.
    let mut by_table: HashMap<&str, Vec<(String, String)>> = HashMap::new();
    for (full_id, name) in entities {
        let Some((table, _id)) = split_full_id(full_id) else {
            continue;
        };
        by_table
            .entry(table)
            .or_default()
            .push((full_id.clone(), name.clone()));
    }

    let mut new_findings = 0;

    for members in by_table.values() {
        // Stage 1: exact grouping on the normalized name.
        let mut norm_groups: HashMap<String, Vec<String>> = HashMap::new();
        for (full_id, name) in members {
            norm_groups
                .entry(naming::normalize(name))
                .or_default()
                .push(full_id.clone());
        }
        let mut exact_pairs: std::collections::HashSet<(String, String)> =
            std::collections::HashSet::new();
        for ids in norm_groups.values() {
            if ids.len() < 2 {
                continue;
            }
            for i in 0..ids.len() {
                for j in (i + 1)..ids.len() {
                    let (a, b) = sorted_pair(ids[i].clone(), ids[j].clone());
                    exact_pairs.insert((a.clone(), b.clone()));
                    new_findings += record_duplicate(db, &a, &b, 1.0).await?;
                }
            }
        }

        // Stage 2: fuzzy scoring for pairs stage 1 didn't already cover.
        for i in 0..members.len() {
            for j in (i + 1)..members.len() {
                let (id_a, name_a) = &members[i];
                let (id_b, name_b) = &members[j];
                let na = naming::normalize(name_a);
                let nb = naming::normalize(name_b);
                if na == nb {
                    continue; // already reported by stage 1
                }
                let (a, b) = sorted_pair(id_a.clone(), id_b.clone());
                if exact_pairs.contains(&(a.clone(), b.clone())) {
                    continue;
                }
                let score = naming::similarity(&na, &nb);
                if score >= naming::DEFAULT_THRESHOLD {
                    new_findings += record_duplicate(db, &a, &b, score).await?;
                }
            }
        }
    }

    Ok(new_findings)
}

/// Writes one deduped `duplicate_entity` finding for the (already-sorted or
/// not — sorted here) pair `a`/`b`. Returns 1 if a new finding was created,
/// 0 if an unresolved finding for this pair already exists (idempotent
/// re-runs never accumulate duplicate findings).
async fn record_duplicate<C: Connection>(
    db: &surrealdb::Surreal<C>,
    a: &str,
    b: &str,
    similarity: f64,
) -> Result<usize, String> {
    let (a, b) = sorted_pair(a.to_string(), b.to_string());
    if finding_exists_2(db, "duplicate_entity", "a", &a, "b", &b).await? {
        return Ok(0);
    }
    record_lint(
        db,
        "duplicate_entity",
        json!({ "a": a, "b": b, "similarity": similarity }),
    )
    .await?;
    Ok(1)
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

/// Display identity for a finding party, looked up fresh at read time.
#[derive(Clone)]
struct Identity {
    name: String,
    aliases: Vec<String>,
    summary: Option<String>,
}

/// Look up name/aliases/summary for a full record id (`kind:id`), skipping
/// soft-deleted rows (`vault_deleted = true`). Returns `None` if the record is
/// missing or deleted — the caller falls back to showing the raw id.
async fn lookup_identity<C: Connection>(
    db: &surrealdb::Surreal<C>,
    full_id: &str,
) -> Result<Option<Identity>, String> {
    let Some((table, id)) = split_full_id(full_id) else {
        return Ok(None);
    };
    let id = strip_backticks(id);
    #[derive(Deserialize)]
    struct Row {
        name: String,
        #[serde(default)]
        aliases: Vec<String>,
        #[serde(default)]
        summary: Option<String>,
    }
    let mut resp = db
        .query(
            "SELECT name, aliases, summary FROM type::thing($tb, $id) \
             WHERE vault_deleted != true",
        )
        .bind(("tb", table.to_owned()))
        .bind(("id", id))
        .await
        .map_err(|e| format!("Failed to look up entity: {e}"))?;
    let rows: Vec<Row> = resp
        .take(0)
        .map_err(|e| format!("Failed to parse entity identity: {e}"))?;
    Ok(rows.into_iter().next().map(|r| Identity {
        name: r.name,
        aliases: r.aliases,
        summary: r.summary,
    }))
}

/// Attach human-readable names (and, for alias collisions, a `*_is_name` flag)
/// to a finding so the Maintenance UI can render conflicts without exposing raw
/// record ids. Other kinds pass through untouched. `cache` memoizes lookups by
/// full id across the whole `list_lint_findings` call so a repeated party
/// (e.g. one entity appearing in several findings) is fetched only once.
async fn enrich_finding_display<C: Connection>(
    db: &surrealdb::Surreal<C>,
    finding: &mut LintFinding,
    cache: &mut HashMap<String, Option<Identity>>,
) -> Result<(), String> {
    if finding.kind != "alias_collision" && finding.kind != "duplicate_entity" {
        return Ok(());
    }
    let key = finding
        .payload
        .get("alias")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let Some(obj) = finding.payload.as_object_mut() else {
        return Ok(());
    };
    for side in ["a", "b"] {
        let Some(full_id) = obj.get(side).and_then(|v| v.as_str()).map(str::to_owned) else {
            continue;
        };
        let identity = if let Some(cached) = cache.get(&full_id) {
            cached.clone()
        } else {
            let looked_up = lookup_identity(db, &full_id).await?;
            cache.insert(full_id.clone(), looked_up.clone());
            looked_up
        };
        if let Some(identity) = identity {
            obj.insert(format!("{side}_name"), json!(identity.name.clone()));
            obj.insert(format!("{side}_summary"), json!(identity.summary));
            // Only alias collisions carry a normalized key to compare against.
            if let Some(k) = key.as_deref() {
                let is_name = naming::normalize(&identity.name) == k;
                obj.insert(format!("{side}_is_name"), json!(is_name));
            }
        }
    }
    Ok(())
}

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
    let mut findings: Vec<LintFinding> = rows
        .into_iter()
        .map(|r| LintFinding {
            id: r.id.id.to_raw(),
            kind: r.kind,
            payload: r.payload,
            created_at: r.created_at.to_string(),
        })
        .collect();
    let mut identity_cache: HashMap<String, Option<Identity>> = HashMap::new();
    for finding in &mut findings {
        enrich_finding_display(db, finding, &mut identity_cache).await?;
    }
    Ok(findings)
}

/// Resolve a naming conflict by keeping the disputed term on `keep_id` and
/// stripping it from `drop_id`. `drop_id` must hold the term as an *alias*; if
/// it is that entity's primary name this errors and mutates nothing (a name
/// cannot be removed — the GM must merge or rename instead). `keep_id` is
/// validated to be the finding's other party but needs no mutation.
pub async fn resolve_alias_collision<C: Connection>(
    db: &surrealdb::Surreal<C>,
    finding_id: &str,
    keep_id: &str,
    drop_id: &str,
) -> Result<(), String> {
    #[derive(Deserialize)]
    struct Row {
        payload: serde_json::Value,
    }
    let mut resp = db
        .query("SELECT payload FROM type::thing('lint_finding', $id)")
        .bind(("id", finding_id.to_owned()))
        .await
        .map_err(|e| format!("Failed to load finding: {e}"))?;
    let rows: Vec<Row> = resp
        .take(0)
        .map_err(|e| format!("Failed to parse finding: {e}"))?;
    let payload = rows
        .into_iter()
        .next()
        .ok_or_else(|| "Finding not found".to_string())?
        .payload;

    let key = payload
        .get("alias")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Finding has no alias key".to_string())?;
    let a = payload.get("a").and_then(|v| v.as_str());
    let b = payload.get("b").and_then(|v| v.as_str());
    let valid = matches!(
        (a, b),
        (Some(x), Some(y))
            if (x == keep_id && y == drop_id) || (x == drop_id && y == keep_id)
    );
    if !valid {
        return Err("keep_id/drop_id do not match this finding".into());
    }

    let identity = lookup_identity(db, drop_id)
        .await?
        .ok_or_else(|| "Losing entity no longer exists".to_string())?;

    // Find the loser's original-cased alias whose normalized form is the key.
    let original = identity
        .aliases
        .iter()
        .find(|al| naming::normalize(al) == key);
    let Some(original) = original else {
        // Not an alias — it must be the primary name (or a stale finding).
        if naming::normalize(&identity.name) == key {
            return Err("Cannot strip a primary name; merge or rename instead".into());
        }
        return Err("Losing entity does not claim this term".into());
    };

    crate::entity_service::remove_alias(db, drop_id, original)
        .await
        .map_err(|e| e.to_string())?;
    resolve_lint_finding(db, finding_id).await
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
