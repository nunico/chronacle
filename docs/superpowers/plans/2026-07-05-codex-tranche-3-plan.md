# Codex Tranche 3 (B3a, B3b, C1a, C1b, C2a, C2b) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the Codex loop: retrieval answers from the compiled layer (B3a rules block, B3b article excerpts), durable results write back through a review queue (C1a proposal producers + accept/reject, C1b Maintenance inbox UI), and drift is surfaced by lint (C2a detectors + pass, C2b findings UI).

**Architecture:** Follows `docs/superpowers/specs/2026-07-03-codex-compiled-world-model-design.md` (sections "Retrieval integration (B3)", "Write-back (C1)", "Linting (C2)", "UX"). B3 lives in `chronacle-retrieval::agent_service` (new `rules_block` module + prompt/context changes). C1/C2 backends extend `chronacle-extraction::codex_service` (new `proposals.rs`, `lint.rs`), reusing `llm_complete`, tolerant JSON parsing, the FLEXIBLE-object struct-bind discipline, and `record_lint` from earlier tranches. UI adds one sidebar item (**Maintenance**, badge = pending proposals + unresolved findings) and one view (`MaintenanceView.svelte`, Proposals tab in C1b, Findings tab in C2b), plus a "Save to Codex" action on assistant chat messages.

**Tech Stack:** Rust (SurrealDB embedded, tokio), Svelte 5 runes + TypeScript, Vitest, playwright-bdd.

## Global Constraints

- Every branch: `git checkout --no-track -b <branch> <base>` — never track main. The tranche is a stacked chain: `feat/b3a-rules-retrieval` from `main`, then each PR branches from its predecessor (`b3b` ← `b3a`, `c1a` ← `b3b`, `c1b` ← `c1a`, `c2a` ← `c1b`, `c2b` ← `c2a`). Rebase-merge repo: after an upstream PR merges, rebase the stack. First push of a stacked branch needs an explicit refspec (`git push -u origin <branch>:refs/heads/<branch>`) — `push.default` would otherwise push to the parent's remote branch.
- No new crates, no new Cargo or npm dependencies anywhere in this tranche. No markdown renderer — articles/diffs render through the existing `WikiText` component or `white-space: pre-wrap` containers.
- Commit subjects ≤ 72 chars, imperative, conventional prefixes; never `--no-verify`.
- Clippy warnings are errors (`cargo clippy --workspace --all-targets --all-features -- -D warnings`); public items need `///` docs; Svelte 5 runes only (`$state`, `$derived`, `$props`) — no `export let` / `$:`.
- BDD (ADR-011): UI-reachable scenarios ship as `.feature` files (C1b, C2b); backend-only scenarios ship as Rust tests named to mirror the Gherkin (B3a, B3b, C1a, C2a), per `apps/desktop/tests/e2e/features/README.md`.
- **KNN + scope (hard rule):** MTREE KNN (`embedding <|K|> $vec`) silently returns 0 rows when combined with `id IN (SELECT …)`. Every scoped KNN in this tranche uses an **inline explicit filter** built into the SQL string (field-equality `collection IN [collection:'a', …]` or graph traversal), mirroring `context/entity.rs`. Single quotes in interpolated ids are escaped `cid.replace('\'', "\\'")`.
- **FLEXIBLE object binding:** never bind a `serde_json::Value` when *writing* to a FLEXIBLE `object` / `array<object>` field — nested keys are lost. Write via plain `#[derive(Serialize)]` structs or inline SurrealQL object literals. *Reading* a FLEXIBLE field into `serde_json::Value` is fine (used for lint payload display).
- **Unset-staleness rule:** rows created pre-migration have `codex_stale` unset (NONE). Needs-compile predicates stay `codex_stale != false OR codex_article = NONE`.
- Machine text reaches the user-owned `notes` field through exactly one path: an accepted `entity_notes_update` proposal. Compiles and all other accept paths never touch `notes`/`summary`.
- LLM/embedding only via `Arc<dyn LlmProvider>` / `Arc<dyn EmbeddingProvider>`; codex tests use `MockLlm` / `MockEmbeddingProvider` from `extraction_service::test_support` (same crate); retrieval tests use the in-crate mocks already used by `context_tests.rs`.
- Frontend `invoke()` argument keys are camelCase (`collectionId`); Tauri maps them to snake_case Rust parameters. Struct arguments need `#[serde(rename_all = "camelCase")]`.
- Each PR ends green on: `cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace && pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend`.

## Shared interfaces introduced by this tranche (single source of truth)

```rust
// crates/chronacle-retrieval/src/agent_service/rules_block.rs        (B3a)
pub(super) const RULES_TOP_K: usize = 5;
pub(super) const RULES_BLOCK_BUDGET: usize = 4_000;   // chars, whole block
pub(super) const RULE_BODY_BUDGET: usize = 1_200;     // chars, per entry
pub async fn fetch_rules_context(db, collection_ids: &[String], query_vec: &[f32])
    -> Result<String, AgentError>;                    // "" when no hits / no collections

// crates/chronacle-retrieval/src/agent_service/prompt.rs             (B3a)
pub(super) fn build_system_prompt(rag_context: &str, entity_context: &str, rules_context: &str)
    -> String;                                        // block order: RULES → CAMPAIGN NOTES → REFERENCE MATERIAL

// crates/chronacle-retrieval/src/agent_service/context/format.rs     (B3b)
pub(super) const ARTICLE_EXCERPT_LEN: usize = 600;    // chars, per entity article excerpt

// crates/chronacle-extraction/src/codex_service/proposals.rs         (C1a)
pub struct ProposalPayload { pub proposed_text: String, pub rationale: String,
                             pub name: Option<String>, pub entity_kind: Option<String>,
                             pub category: Option<String> }
pub struct CodexProposal   { pub id: String, pub kind: String, pub target: Option<String>,
                             pub target_name: Option<String>, pub current_text: Option<String>,
                             pub payload: ProposalPayload, pub origin_kind: String,
                             pub status: String, pub created_at: String }
pub const MAX_PROPOSALS_PER_DISTILL: usize = 8;
pub async fn distill_chat_answer(db, llm, campaign_id: &str, answer: &str) -> Result<usize, CodexError>;
pub async fn distill_session_notes(db, llm, session_id: &str) -> Result<usize, CodexError>;
pub async fn list_proposals(db, status: Option<&str>) -> Result<Vec<CodexProposal>, String>;
pub async fn accept_proposal(db, embed, proposal_id: &str) -> Result<(), String>;
pub async fn reject_proposal(db, proposal_id: &str) -> Result<(), String>;
pub async fn maintenance_counts(db) -> Result<MaintenanceCounts, String>;
pub struct MaintenanceCounts { pub pending_proposals: usize, pub unresolved_findings: usize }

// crates/chronacle-extraction/src/codex_service/lint.rs              (C2a)
pub struct LintSummary { pub new_findings: usize, pub unresolved_total: usize }
pub struct LintFinding { pub id: String, pub kind: String, pub payload: serde_json::Value,
                         pub created_at: String }
pub async fn run_lint_campaign(db, campaign_id: &str) -> Result<LintSummary, String>;
pub async fn run_lint_collection(db, collection_id: &str) -> Result<LintSummary, String>;
pub async fn list_lint_findings(db) -> Result<Vec<LintFinding>, String>;
pub async fn resolve_lint_finding(db, finding_id: &str) -> Result<(), String>;
```

```ts
// apps/desktop/src/lib/commands.ts additions
export interface ProposalPayload { proposed_text: string; rationale: string;
  name: string | null; entity_kind: string | null; category: string | null; }
export interface CodexProposal { id: string; kind: string; target: string | null;
  target_name: string | null; current_text: string | null; payload: ProposalPayload;
  origin_kind: string; status: string; created_at: string; }
export interface MaintenanceCounts { pending_proposals: number; unresolved_findings: number; }
export interface LintFinding { id: string; kind: string; payload: Record<string, unknown>; created_at: string; }
export interface LintSummary { new_findings: number; unresolved_total: number; }
saveChatToCodex(campaignId: string, content: string): Promise<number>   // C1b
getProposals(status?: string): Promise<CodexProposal[]>                 // C1b
acceptProposal(id: string): Promise<void>                               // C1b
rejectProposal(id: string): Promise<void>                               // C1b
getMaintenanceCounts(): Promise<MaintenanceCounts>                      // C1b
runLint(campaignId: string): Promise<LintSummary>                       // C2b
getLintFindings(): Promise<LintFinding[]>                               // C2b
resolveLintFinding(id: string): Promise<void>                           // C2b
deleteRelation(edgeId: string): Promise<void>                           // C2b
```

Frontend view plumbing (C1b): `CampaignRail.svelte`'s `View` union gains `'maintenance'`; the rail shows a **Maintenance** nav item with a pending-count badge; `Shell.svelte` renders `MaintenanceView` and refreshes the count after inbox actions.

---

# PR B3a — `feat/b3a-rules-retrieval`

RULES block: `rule_entry` KNN across the campaign's subscribed collections, budget-capped rendering, prompt block ordering RULES → CAMPAIGN NOTES → REFERENCE MATERIAL, citation instructions for rules.

### Task 1: `rules_block` module — fetch + format with budgets

**Files:**
- Create: `crates/chronacle-retrieval/src/agent_service/rules_block.rs`
- Modify: `crates/chronacle-retrieval/src/agent_service/mod.rs` (register module, call in pipeline)

**Interfaces:**
- Consumes: `AgentError` (existing), `rule_entry` schema (A2a), `RulePageRef` shape (`{source_name, page_start, page_end}` objects in `page_refs`).
- Produces: `fetch_rules_context(db, collection_ids, query_vec) -> Result<String, AgentError>` (empty string ⇒ no block), `format_rules_block(&[RuleHit]) -> String`, constants `RULES_TOP_K`, `RULES_BLOCK_BUDGET`, `RULE_BODY_BUDGET`.

- [ ] **Step 1: Create the branch**

```bash
git checkout --no-track -b feat/b3a-rules-retrieval main
```

- [ ] **Step 2: Write failing unit tests for the pure formatter** (bottom of the new `rules_block.rs`; the module skeleton contains only the types so the tests compile but fail)

```rust
//! RULES context block: KNN over compiled `rule_entry` rows scoped to the
//! campaign's subscribed collections, rendered budget-capped for the prompt.

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
        assert!(out.chars().count() < RULE_BODY_BUDGET + 300, "body must be excerpted");
        assert!(out.contains('…'));
    }

    #[test]
    fn format_stops_at_block_budget() {
        let body = "y".repeat(RULE_BODY_BUDGET);
        let hits: Vec<RuleHit> = (0..10).map(|i| hit(&format!("R{i}"), &body, None)).collect();
        let out = format_rules_block(&hits);
        assert!(out.chars().count() <= RULES_BLOCK_BUDGET + RULE_BODY_BUDGET,
            "block must cut off near the budget, got {}", out.chars().count());
        assert!(!out.contains("R9"), "later entries must be dropped once over budget");
    }

    #[test]
    fn single_page_refs_collapse() {
        let mut h = hit("One", "b", None);
        h.page_refs[0].page_end = 14;
        let out = format_rules_block(&[h]);
        assert!(out.contains("PHB p.14"));
        assert!(!out.contains("p.14-14"));
    }
}
```

- [ ] **Step 3: Register the module and run the tests to verify they fail**

In `agent_service/mod.rs` add below the existing `mod` lines:

```rust
mod rules_block;
```

Run: `cargo test -p chronacle-retrieval rules_block -- --nocapture`
Expected: FAIL — `format_rules_block` not found.

- [ ] **Step 4: Implement `format_rules_block` (pure) in `rules_block.rs`**

```rust
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
```

- [ ] **Step 5: Run the formatter tests to verify they pass**

Run: `cargo test -p chronacle-retrieval rules_block -- --nocapture`
Expected: PASS (all five).

- [ ] **Step 6: Write the failing KNN integration test** (same file, inside `mod tests`; in-memory SurrealDB + migrations, mirroring `codex_service` test setup)

```rust
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
        assert!(ctx.contains("Initiative"), "in-scope rule must be retrieved: {ctx}");
        assert!(ctx.contains("PHB p.14-15"));
        assert!(!ctx.contains("Stealth"), "out-of-scope rule must be filtered: {ctx}");

        let empty = fetch_rules_context(&db, &[], &in_vec).await.unwrap();
        assert_eq!(empty, "", "no collections ⇒ no block");
    }
```

Add `chronacle-db` to `[dev-dependencies]` in `crates/chronacle-retrieval/Cargo.toml` **only if it is not already there** (check first — `context_tests` may already use it; workspace-internal crates need no ADR).

Run: `cargo test -p chronacle-retrieval fetch_rules_context -- --nocapture`
Expected: FAIL — `fetch_rules_context` not found.

- [ ] **Step 7: Implement `fetch_rules_context`**

```rust
/// KNN top-[`RULES_TOP_K`] rule entries across `collection_ids`, formatted as
/// the RULES prompt block. Returns an empty string when there are no
/// collections, no embedded entries, or no hits.
///
/// The collection filter is an inline explicit array (`collection IN [...]`)
/// because MTREE KNN silently returns zero rows when combined with an
/// `id IN (SELECT …)` subquery.
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
    let sql = format!(
        "SELECT name, category, body, notes, page_refs, \
             vector::distance::knn() AS distance \
         FROM rule_entry \
         WHERE embedding <|{RULES_TOP_K}|> [{vec_str}] AND collection IN [{cols}] \
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
```

- [ ] **Step 8: Run tests, then commit**

Run: `cargo test -p chronacle-retrieval rules_block -- --nocapture` → PASS.

```bash
git add crates/chronacle-retrieval
git commit -m "feat(retrieval): rules-block KNN with scoped explicit filter"
```

### Task 2: Prompt assembly — RULES block first, rules citation instruction

**Files:**
- Modify: `crates/chronacle-retrieval/src/agent_service/prompt.rs`
- Modify: `crates/chronacle-retrieval/src/agent_service/mod.rs`

**Interfaces:**
- Produces: `build_system_prompt(rag_context, entity_context, rules_context) -> String`; `stream_response` fetches the rules context for campaign chats and passes it through.

- [ ] **Step 1: Write failing prompt tests** (append to `prompt.rs` `mod tests`)

```rust
    #[test]
    fn rules_block_leads_and_carries_citation_instruction() {
        let rules = "COMPILED RULES (distilled from your rulebooks):\n\n[mechanic] Initiative — PHB p.14\nRoll d20.\n\n";
        let rag = "[0] Source: \"PHB\", p.1 — \"Intro\"\nSome rules.\n\n";
        let ent = "Campaign notes (your GM records):\n\n[npc] Aldric\n";
        let prompt = build_system_prompt(rag, ent, rules);
        let i_rules = prompt.find("COMPILED RULES").expect("rules section");
        let i_notes = prompt.find("CAMPAIGN NOTES").expect("notes section");
        let i_rag = prompt.find("REFERENCE MATERIAL").expect("rag section");
        assert!(i_rules < i_notes && i_notes < i_rag,
            "block order must be RULES → CAMPAIGN NOTES → REFERENCE MATERIAL");
        assert!(prompt.contains("COMPILED RULES cite the book and page"),
            "rules claims must carry a citation instruction");
    }

    #[test]
    fn no_rules_context_is_todays_behavior() {
        let rag = "[0] Source: \"PHB\", p.1 — \"Intro\"\nSome rules.\n\n";
        let with = build_system_prompt(rag, "", "");
        assert!(!with.contains("COMPILED RULES"));
        assert!(with.contains("REFERENCE MATERIAL"), "regression: rag-only unchanged");
    }
```

Update **every existing call/test** of `build_system_prompt(a, b)` to `build_system_prompt(a, b, "")` so the crate compiles.

Run: `cargo test -p chronacle-retrieval prompt -- --nocapture`
Expected: FAIL on the two new tests.

- [ ] **Step 2: Implement — extend `build_system_prompt`**

In `prompt.rs`, change the signature and add the block + instruction:

```rust
pub(super) fn build_system_prompt(
    rag_context: &str,
    entity_context: &str,
    rules_context: &str,
) -> String {
    let has_rag = !rag_context.is_empty();
    let has_entities = !entity_context.is_empty();
    let has_rules = !rules_context.is_empty();

    if !has_rag && !has_entities && !has_rules {
        return "You are an expert Game Master assistant. \
            Answer the user's question to the best of your ability. \
            If you don't know the answer, say so — do not make up rules."
            .to_string();
    }

    let mut prompt = String::from("You are an expert Game Master assistant.\n\n");

    if has_rules {
        prompt.push_str(&format!("{rules_context}\n"));
    }
    // …existing has_rag / has_entities pushes unchanged, keeping their order…
```

and inside the `INSTRUCTIONS` section add (after the existing `has_rag` citation instruction block):

```rust
    if has_rules {
        prompt.push_str(
            "- Claims taken from COMPILED RULES cite the book and page shown on the entry, \
             using the same [Source: \"<source name>\", p.<page>, quote: \"<verbatim sentence>\"] \
             format; quote the sentence from the entry body that supports the claim. \
             Lines labeled \"GM table ruling\" are the GM's own house rulings — prefer them \
             over book text when they conflict, and attribute them as the GM's ruling.\n",
        );
    }
```

- [ ] **Step 3: Wire into `stream_response`** (`mod.rs`, after the entity-context fetch, before the vector search)

```rust
    let rules_context = match campaign_id {
        Some(_) if !collection_ids.is_empty() => {
            rules_block::fetch_rules_context(db, &collection_ids, &query_vector)
                .await
                .unwrap_or_else(|e| {
                    eprintln!("rules context fetch failed: {e}");
                    String::new()
                })
        }
        _ => String::new(),
    };
```

and change the prompt call:

```rust
    let system_prompt = prompt::build_system_prompt(&context, &entity_context, &rules_context);
```

- [ ] **Step 4: Verify, then commit**

Run: `cargo test -p chronacle-retrieval -- --nocapture` → PASS (including the untouched regression tests — no-campaign chat still skips everything).

```bash
git add crates/chronacle-retrieval
git commit -m "feat(retrieval): RULES prompt block with citation instruction"
```

### Task 3: B3a acceptance tests (Rust, Gherkin-mirroring) + docs + PR

**Files:**
- Modify: `apps/desktop/src-tauri/tests/` — add `codex_retrieval_test.rs`
- Modify: `docs/architecture.md` (RAG pipeline section: block ordering note)

- [ ] **Step 1: Write the backend-only acceptance test** (`apps/desktop/src-tauri/tests/codex_retrieval_test.rs`) — names mirror the B3 Gherkin

```rust
//! B3 acceptance (ADR-011, backend-only): compiled rules reach the prompt in
//! RULES → CODEX/ENTITIES → CHUNKS order; uncompiled campaigns are unchanged.
//! Scenario names mirror the spec's Gherkin.

use std::sync::{Arc, RwLock};

// Reuse the integration-test helpers other tests in this dir use for the
// in-memory DB + mock providers (see existing tests for the exact imports;
// e.g. `common` module or inline mocks matching chat_pipeline tests).
```

The test body (follow the established pattern in the existing integration tests for building `Surreal<Db>` + mock `LlmProvider` that records its system prompt + mock embedding + `SurrealDbVector`):

```rust
#[tokio::test]
async fn rules_question_gets_rules_block_before_chunks() {
    // Given compiled rules and an indexed chunk in a subscribed collection
    // (seed campaign camp1 -> subscribes_to -> collection ca; rule_entry with
    //  embedding; chunk with embedding), When stream_response runs,
    // Then the recorded system prompt contains COMPILED RULES before
    // REFERENCE MATERIAL and includes the book+page of the rule entry.
}

#[tokio::test]
async fn campaign_with_no_compiled_content_behaves_exactly_as_today() {
    // Given a campaign with chunks but zero rule_entry rows and no articles,
    // the system prompt has no COMPILED RULES section and the chunk block is
    // unchanged (regression guard).
}
```

Implement both fully using a recording mock LLM (an `Arc<Mutex<Option<String>>>` the mock writes the received system prompt into — the same technique the existing chat integration tests use; if none exists, write the small mock in this file).

Run: `cargo test -p chronacle-desktop --test codex_retrieval_test -- --nocapture`
Expected: PASS.

- [ ] **Step 2: Update `docs/architecture.md`** — in the RAG pipeline section add one short paragraph: prompt block order is now RULES → CODEX/ENTITIES → CHUNKS (ADR-009, B3); rules block budgeted at 4 000 chars, top-5 KNN over `rule_entry` scoped by explicit collection array.

- [ ] **Step 3: Full gate, push, PR**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace
pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend
git add -A && git commit -m "test(e2e): B3a rules-retrieval acceptance; architecture note"
git push -u origin feat/b3a-rules-retrieval:refs/heads/feat/b3a-rules-retrieval
gh pr create --base main --head feat/b3a-rules-retrieval --title "feat(retrieval): RULES block from compiled rule entries (B3a)" --body "..."
```

PR body: what/why/how-tested + link to the spec.

---

# PR B3b — `feat/b3b-codex-retrieval`

Codex article excerpts enrich the entity context; ordering/budget tests.

Branch: `git checkout --no-track -b feat/b3b-codex-retrieval feat/b3a-rules-retrieval`

### Task 4: Article excerpts in entity context

**Files:**
- Modify: `crates/chronacle-retrieval/src/agent_service/context/rows.rs` (add `codex_article` to `BasicRow`, `EventRow`, `PcRow`)
- Modify: `crates/chronacle-retrieval/src/agent_service/context/entity.rs` (SELECT lists)
- Modify: `crates/chronacle-retrieval/src/agent_service/context/format.rs` (excerpt rendering + budget)
- Test: `crates/chronacle-retrieval/src/agent_service/context/context_tests_entity.rs`

**Interfaces:**
- Consumes: `codex_article` field on all 8 entity tables (A2a).
- Produces: entities with a non-empty article render `· Codex: <first ARTICLE_EXCERPT_LEN chars>` **instead of** `· <summary>`; entities without keep today's output byte-for-byte.

- [ ] **Step 1: Write failing tests** (append to `context_tests_entity.rs`, following its existing seeding helpers)

```rust
#[tokio::test]
async fn entity_with_codex_article_contributes_excerpt_instead_of_summary() {
    // Seed campaign npc with summary='Old summary' and
    // codex_article='Compiled article text …' (longer than ARTICLE_EXCERPT_LEN
    // to also pin truncation+ellipsis), fetch_entity_context, assert:
    //   out.contains("Codex: Compiled article text")
    //   !out.contains("Old summary")
    //   excerpt length ≤ ARTICLE_EXCERPT_LEN + 1 ('…')
}

#[tokio::test]
async fn entity_without_article_renders_exactly_as_before() {
    // Seed npc with summary only; assert the exact pre-B3b line format
    // "[npc] Name · Summary" still appears (regression guard).
}
```

Run: `cargo test -p chronacle-retrieval context_tests_entity -- --nocapture` → FAIL.

- [ ] **Step 2: Implement**

`rows.rs` — add to `BasicRow`, `EventRow`, `PcRow`:

```rust
    #[serde(default)]
    pub codex_article: Option<String>,
```

`entity.rs` — add `codex_article` to every SELECT list (all 8 campaign queries, the PC/event variants, and both branches of the collection-entity SQL: KNN and fallback).

`format.rs` — add the constant and helper:

```rust
/// Max characters of a codex article included per entity in the context block.
pub(super) const ARTICLE_EXCERPT_LEN: usize = 600;

/// Article excerpt for the context line, or `None` when absent/blank.
pub(super) fn article_excerpt(article: Option<&str>) -> Option<String> {
    let trimmed = article?.trim();
    if trimmed.is_empty() {
        return None;
    }
    let collapsed = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= ARTICLE_EXCERPT_LEN {
        Some(collapsed)
    } else {
        let cut: String = collapsed.chars().take(ARTICLE_EXCERPT_LEN).collect();
        Some(format!("{cut}…"))
    }
}
```

In `format_entity_output`, for each row-rendering site (`PcRow`, the five `BasicRow` loops, `EventRow`, `misc`, and the collection-entities loop), replace the summary push with:

```rust
            if let Some(a) = article_excerpt(r.codex_article.as_deref()) {
                out.push_str(&format!(" · Codex: {a}"));
            } else if let Some(s) = &r.summary {
                if !s.trim().is_empty() {
                    out.push_str(&format!(" · {s}"));
                }
            }
```

(Notes excerpts stay appended after, unchanged.)

- [ ] **Step 3: Verify + commit**

Run: `cargo test -p chronacle-retrieval -- --nocapture` → PASS.

```bash
git add crates/chronacle-retrieval
git commit -m "feat(retrieval): codex article excerpts in entity context"
```

### Task 5: B3b acceptance test + PR

**Files:**
- Modify: `apps/desktop/src-tauri/tests/codex_retrieval_test.rs`

- [ ] **Step 1: Add the ordering acceptance test** (backend-only Gherkin mirror): `compiled_article_excerpt_appears_in_codex_block_between_rules_and_chunks` — seed a compiled article + compiled rule + chunk; assert prompt index order `COMPILED RULES < Codex: < REFERENCE MATERIAL`.

- [ ] **Step 2: Full gate, push, stacked PR**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace
pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend
git add -A && git commit -m "test(e2e): B3b codex-excerpt ordering acceptance"
git push -u origin feat/b3b-codex-retrieval:refs/heads/feat/b3b-codex-retrieval
gh pr create --base feat/b3a-rules-retrieval --head feat/b3b-codex-retrieval --title "feat(retrieval): codex article excerpts in entity context (B3b)" --body "..."
```

---

# PR C1a — `feat/c1a-proposals-backend`

Proposal producers (chat distillation, session-notes pass), list/accept/reject service, maintenance counts, Tauri commands, session-save hook.

Branch: `git checkout --no-track -b feat/c1a-proposals-backend feat/b3b-codex-retrieval`

### Task 6: Proposal types, parsing, and creation

**Files:**
- Create: `crates/chronacle-extraction/src/codex_service/proposals.rs`
- Create: `crates/chronacle-extraction/src/codex_service/proposals_tests.rs`
- Modify: `crates/chronacle-extraction/src/codex_service/mod.rs` (register + re-export)
- Modify: `crates/chronacle-extraction/src/codex_service/prompts.rs` (distillation prompts)
- Modify: `crates/chronacle-extraction/src/wikilink/mod.rs` + `query.rs` (make `query_all_entity_names` `pub(crate)`)

**Interfaces:**
- Consumes: `llm_complete` (extraction_service), `strip_code_fences` idiom (duplicate locally like `rules.rs` did), `codex_proposal` schema (A2a), `mark_entity_stale`, `wikilink::WikilinkScope` + `query_all_entity_names` (visibility widened to `pub(crate)`).
- Produces: `ProposalPayload`, `CodexProposal`, `distill_chat_answer`, `distill_session_notes`, `MAX_PROPOSALS_PER_DISTILL` (see Shared interfaces).

- [ ] **Step 1: Widen wikilink query visibility**

In `wikilink/query.rs` change `pub(super) async fn query_all_entity_names` to `pub(crate) async fn query_all_entity_names`; in `wikilink/mod.rs` add `pub(crate) use query::query_all_entity_names;` (keep `WikilinkScope` as-is — it is already `pub`).

- [ ] **Step 2: Write failing tests for parsing + persistence** (`proposals_tests.rs`; register in `mod.rs` under `#[cfg(test)]`)

```rust
//! Tests for proposal distillation, resolution, and the accept/reject service.

use std::sync::Arc;

use surrealdb::engine::local::{Db, Mem};
use surrealdb::Surreal;

use super::proposals::*;
use crate::extraction_service::test_support::{MockEmbeddingProvider, MockLlm};

async fn setup_db() -> Surreal<Db> {
    let db = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db
}

/// Seed campaign `camp1` with an owned collection `own1`, a subscription to it,
/// and one npc `Mira` in `own1`.
async fn seed_campaign(db: &Surreal<Db>) {
    db.query(
        "CREATE campaign:`camp1` SET name='C', system='5e', created_at=time::now(), updated_at=time::now();
         CREATE collection:`own1` SET name='C — Notes', description=NULL, owner_campaign=campaign:`camp1`,
             created_at=time::now(), updated_at=time::now();
         RELATE campaign:`camp1`->subscribes_to->collection:`own1` SET created_at=time::now();
         CREATE npc:`mira` SET name='Mira', summary='A sage', notes=NULL,
             created_at=time::now(), updated_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`mira` SET created_at=time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();
}

#[tokio::test]
async fn distill_chat_answer_creates_targeted_pending_proposals() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    let llm: Arc<dyn chronacle_core::llm::LlmProvider> = Arc::new(MockLlm::with_response(
        r#"{"proposals":[
            {"kind":"entity_article_update","target_name":"Mira",
             "proposed_text":"Mira is the sage of Vethara.","rationale":"Answer established her origin."},
            {"kind":"new_entity","target_name":"Vethara","entity_kind":"location",
             "proposed_text":"A mountain city.","rationale":"New place named in the answer."}
        ]}"#,
    ));
    let n = distill_chat_answer(&db, &llm, "camp1", "Mira hails from Vethara …")
        .await
        .unwrap();
    assert_eq!(n, 2);

    let rows = list_proposals(&db, Some("pending")).await.unwrap();
    assert_eq!(rows.len(), 2);
    let update = rows.iter().find(|p| p.kind == "entity_article_update").unwrap();
    assert_eq!(update.target_name.as_deref(), Some("Mira"));
    assert!(update.target.as_deref().unwrap().starts_with("npc:"));
    assert_eq!(update.origin_kind, "chat");
    let fresh = rows.iter().find(|p| p.kind == "new_entity").unwrap();
    assert!(fresh.target.is_none());
    assert_eq!(fresh.payload.entity_kind.as_deref(), Some("location"));
}

#[tokio::test]
async fn distill_skips_unresolvable_update_targets_and_caps_output() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    // 10 proposals: 1 unresolvable update + 9 new entities; cap is 8.
    let mut items = vec![
        r#"{"kind":"entity_notes_update","target_name":"Nobody","proposed_text":"x","rationale":"r"}"#.to_string(),
    ];
    for i in 0..9 {
        items.push(format!(
            r#"{{"kind":"new_entity","target_name":"E{i}","entity_kind":"npc","proposed_text":"t","rationale":"r"}}"#
        ));
    }
    let json = format!(r#"{{"proposals":[{}]}}"#, items.join(","));
    let llm: Arc<dyn chronacle_core::llm::LlmProvider> = Arc::new(MockLlm::with_response(&json));
    let n = distill_chat_answer(&db, &llm, "camp1", "answer").await.unwrap();
    assert_eq!(n, MAX_PROPOSALS_PER_DISTILL, "capped and unresolvable skipped");
}

#[tokio::test]
async fn garbage_llm_output_yields_zero_proposals_not_an_error() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    let llm: Arc<dyn chronacle_core::llm::LlmProvider> =
        Arc::new(MockLlm::with_response("not json at all"));
    let n = distill_chat_answer(&db, &llm, "camp1", "answer").await.unwrap();
    assert_eq!(n, 0);
}
```

> If `MockLlm` lacks a `with_response(&str)` constructor, add one to `extraction_service/test_support.rs` (fixed-response mock) in this step — same crate, test-only.

Run: `cargo test -p chronacle-extraction proposals -- --nocapture` → FAIL (module missing).

- [ ] **Step 3: Add distillation prompts** (`prompts.rs`)

```rust
/// Build the prompt that distills a chat answer into targeted codex proposals.
///
/// `known_entities` are the in-scope entity names (with kinds) so the LLM can
/// target existing records; anything else becomes a `new_entity` draft.
pub(super) fn build_chat_distill_prompt(answer: &str, known_entities: &str) -> String {
    format!(
        r#"You are maintaining a TTRPG campaign codex. A cited answer was just given to the GM.
Distill it into zero or more SMALL, TARGETED update proposals for the codex. Only propose changes
that add durable knowledge — skip restatements of what the codex already implies, greetings, or
speculation. Never invent facts not present in the answer.

Proposal kinds:
- entity_article_update: improve an existing entity's compiled article (target an entity below).
- entity_notes_update: suggest an addition to the GM's own notes on an entity (rare; only for
  table-decision-like facts).
- new_entity: a person/place/faction/creature/item/event named in the answer but missing below.
  Set entity_kind to one of: npc, location, faction, creature, item, event, misc.
- rule_entry_update / new_rule_entry: only for rules content, with category one of:
  mechanic, ability, state, procedure, resource, statistic, entry.

Known entities (name — kind):
{known_entities}

Return ONLY JSON, no prose, no markdown fences:
{{ "proposals": [ {{ "kind": "…", "target_name": "…", "entity_kind": null, "category": null,
                   "proposed_text": "…", "rationale": "…" }} ] }}

The answer:
{answer}"#
    )
}

/// Build the prompt that distills saved session notes into proposals and a
/// mentioned-entity list (used to mark staleness).
pub(super) fn build_session_distill_prompt(notes: &str, known_entities: &str) -> String {
    format!(
        r#"You are maintaining a TTRPG campaign codex. The GM just saved session notes.
Extract durable knowledge: propose entity article updates for entities whose story moved, and
new_entity drafts for people/places/things that appear in the notes but not in the known list.
Also list EVERY known entity mentioned in the notes (exact names from the list).

Known entities (name — kind):
{known_entities}

Return ONLY JSON, no prose, no markdown fences:
{{ "proposals": [ {{ "kind": "entity_article_update|new_entity", "target_name": "…",
                   "entity_kind": null, "proposed_text": "…", "rationale": "…" }} ],
  "mentioned": [ "…" ] }}

Session notes:
{notes}"#
    )
}
```

- [ ] **Step 4: Implement `proposals.rs` — types, parsing, resolution, creation**

```rust
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
```

Resolution + creation helpers (same file):

```rust
/// The campaign's owned collection (ADR-010 auto-owned notes collection).
async fn owned_collection_id<C: Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
) -> Result<Option<String>, CodexError> {
    #[derive(Deserialize)]
    struct Row { id: Thing }
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
```

```rust
/// Persist one resolved proposal. `target` is a full "table:id" or None.
/// `origin_extra` is an optional (key, Thing) appended into origin.
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
            format!("type::thing('{}', '{}')", table.replace('\'', ""), id.replace('\'', ""))
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
```

Then the shared distill core and the two producers:

```rust
/// Case-insensitive name → "table:id" map of the campaign's in-scope entities.
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
        let target = match (&d.target_name, is_new) {
            (Some(n), false) => match resolve_target(&known, n) {
                Some(t) => Some(t.to_string()),
                None => {
                    eprintln!("codex: skipping proposal for unknown target '{n}'");
                    continue;
                }
            },
            _ => None,
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
```

(`origin_ref: Option<(&str, String)>` needs `Clone` — it is, being `(&str, String)`.)

```rust
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
    let messages = vec![ChatMessage { role: "user".to_string(), content: prompt }];
    let raw = llm_complete(llm.as_ref(), SYSTEM_PROMPT, &messages)
        .await
        .map_err(|e| CodexError::Llm(e.to_string()))?;
    let parsed = parse_distill_response(&raw);

    // Best-effort origin: the persisted assistant message with this content.
    #[derive(Deserialize)]
    struct MsgRow { id: Thing }
    let mut resp = db
        .query(
            "SELECT id FROM message WHERE role = 'assistant' AND content = $c \
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
    struct SessionRow { campaign: Option<Thing>, notes: String }
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
    let Some(campaign) = session.campaign else { return Ok(0) };
    let campaign_id = campaign.id.to_raw();
    if session.notes.trim().is_empty() {
        return Ok(0);
    }

    // Replace this session's previous pending proposals (idempotent re-save).
    db.query(
        "DELETE codex_proposal WHERE status = 'pending' \
             AND origin.session = $sid AND origin.kind = 'session'",
    )
    .bind(("sid", session_id.to_owned()))
    .await
    .map_err(|e| CodexError::Db(e.to_string()))?;

    let known = known_entities(db, &campaign_id).await?;
    let known_block = known
        .iter()
        .map(|(id, n)| format!("- {n} — {}", id.split(':').next().unwrap_or("?")))
        .collect::<Vec<_>>()
        .join("\n");
    let prompt = build_session_distill_prompt(&session.notes, &known_block);
    let messages = vec![ChatMessage { role: "user".to_string(), content: prompt }];
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
```

Register in `codex_service/mod.rs`:

```rust
mod proposals;
#[cfg(test)]
mod proposals_tests;
pub use proposals::{
    accept_proposal, distill_chat_answer, distill_session_notes, list_proposals,
    maintenance_counts, reject_proposal, CodexProposal, MaintenanceCounts, ProposalPayload,
    MAX_PROPOSALS_PER_DISTILL,
};
```

(`list_proposals`/`accept`/`reject`/`maintenance_counts` are Task 7 — add a temporary `todo!()`-free stub only if needed to compile, or write Task 7 before running the full suite; prefer implementing Task 7 next in the same session.)

- [ ] **Step 5: Run the Task 6 tests**

Run: `cargo test -p chronacle-extraction proposals -- --nocapture`
Expected: the three distillation tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/chronacle-extraction
git commit -m "feat(codex): proposal distillation from chat and session notes"
```

### Task 7: List / accept / reject / counts

**Files:**
- Modify: `crates/chronacle-extraction/src/codex_service/proposals.rs`
- Modify: `crates/chronacle-extraction/src/codex_service/rules.rs` (make `embed_rule_entry` `pub(super)`)
- Test: `crates/chronacle-extraction/src/codex_service/proposals_tests.rs`

**Interfaces:**
- Consumes: `embed_entity_with_article` (pub(crate), compile.rs), `embed_rule_entry` (rules.rs, widen to `pub(super)`), `entity_service::{create, get_by_id, EntityKind, EntityInput}`.
- Produces: `list_proposals`, `accept_proposal`, `reject_proposal`, `maintenance_counts` (see Shared interfaces).

- [ ] **Step 1: Write failing tests** (append to `proposals_tests.rs`)

```rust
#[tokio::test]
async fn accept_article_update_applies_text_provenance_and_resolves() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    let llm: Arc<dyn chronacle_core::llm::LlmProvider> = Arc::new(MockLlm::with_response(
        r#"{"proposals":[{"kind":"entity_article_update","target_name":"Mira",
            "proposed_text":"Mira, sage of Vethara.","rationale":"r"}]}"#,
    ));
    distill_chat_answer(&db, &llm, "camp1", "answer").await.unwrap();
    let id = list_proposals(&db, Some("pending")).await.unwrap()[0].id.clone();

    let embed: Arc<dyn chronacle_core::embedding::EmbeddingProvider> =
        Arc::new(MockEmbeddingProvider::new(768));
    accept_proposal(&db, &embed, &id).await.unwrap();

    #[derive(serde::Deserialize)]
    struct Npc { codex_article: Option<String>, codex_stale: Option<bool>,
                 codex_sources: Vec<serde_json::Value> }
    let mut r = db.query("SELECT codex_article, codex_stale, codex_sources FROM npc:`mira`")
        .await.unwrap();
    let npc: Option<Npc> = r.take(0).unwrap();
    let npc = npc.unwrap();
    assert_eq!(npc.codex_article.as_deref(), Some("Mira, sage of Vethara."));
    assert_eq!(npc.codex_stale, Some(false), "direct article write is not stale");
    assert!(npc.codex_sources.iter().any(|s| s["kind"] == "proposal"),
        "provenance appended: {:?}", npc.codex_sources);

    let pending = list_proposals(&db, Some("pending")).await.unwrap();
    assert!(pending.is_empty());
    let accepted = list_proposals(&db, Some("accepted")).await.unwrap();
    assert_eq!(accepted.len(), 1);
}

#[tokio::test]
async fn accept_notes_update_is_the_only_machine_path_into_notes_and_marks_stale() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    let llm: Arc<dyn chronacle_core::llm::LlmProvider> = Arc::new(MockLlm::with_response(
        r#"{"proposals":[{"kind":"entity_notes_update","target_name":"Mira",
            "proposed_text":"Party owes Mira a favor.","rationale":"r"}]}"#,
    ));
    distill_chat_answer(&db, &llm, "camp1", "a").await.unwrap();
    let id = list_proposals(&db, Some("pending")).await.unwrap()[0].id.clone();
    let embed: Arc<dyn chronacle_core::embedding::EmbeddingProvider> =
        Arc::new(MockEmbeddingProvider::new(768));
    accept_proposal(&db, &embed, &id).await.unwrap();

    #[derive(serde::Deserialize)]
    struct Npc { notes: Option<String>, codex_stale: Option<bool> }
    let mut r = db.query("SELECT notes, codex_stale FROM npc:`mira`").await.unwrap();
    let npc: Option<Npc> = r.take(0).unwrap();
    let npc = npc.unwrap();
    assert_eq!(npc.notes.as_deref(), Some("Party owes Mira a favor."));
    assert_eq!(npc.codex_stale, Some(true), "notes edit marks the article stale");
}

#[tokio::test]
async fn accept_new_entity_creates_it_in_the_proposal_collection() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    let llm: Arc<dyn chronacle_core::llm::LlmProvider> = Arc::new(MockLlm::with_response(
        r#"{"proposals":[{"kind":"new_entity","target_name":"Vethara","entity_kind":"location",
            "proposed_text":"A mountain city.","rationale":"r"}]}"#,
    ));
    distill_chat_answer(&db, &llm, "camp1", "a").await.unwrap();
    let id = list_proposals(&db, Some("pending")).await.unwrap()[0].id.clone();
    let embed: Arc<dyn chronacle_core::embedding::EmbeddingProvider> =
        Arc::new(MockEmbeddingProvider::new(768));
    accept_proposal(&db, &embed, &id).await.unwrap();

    #[derive(serde::Deserialize)]
    struct Row { name: String }
    let mut r = db
        .query("SELECT name FROM location WHERE <-in_collection<-collection CONTAINS collection:`own1`")
        .await.unwrap();
    let rows: Vec<Row> = r.take(0).unwrap();
    assert!(rows.iter().any(|l| l.name == "Vethara"), "{rows:?}" );
}

#[tokio::test]
async fn reject_changes_nothing_and_resolves() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    let llm: Arc<dyn chronacle_core::llm::LlmProvider> = Arc::new(MockLlm::with_response(
        r#"{"proposals":[{"kind":"entity_article_update","target_name":"Mira",
            "proposed_text":"X","rationale":"r"}]}"#,
    ));
    distill_chat_answer(&db, &llm, "camp1", "a").await.unwrap();
    let id = list_proposals(&db, Some("pending")).await.unwrap()[0].id.clone();
    reject_proposal(&db, &id).await.unwrap();

    #[derive(serde::Deserialize)]
    struct Npc { codex_article: Option<String> }
    let mut r = db.query("SELECT codex_article FROM npc:`mira`").await.unwrap();
    let npc: Option<Npc> = r.take(0).unwrap();
    assert!(npc.unwrap().codex_article.is_none(), "reject must not touch the target");
    assert_eq!(maintenance_counts(&db).await.unwrap().pending_proposals, 0);
}

#[tokio::test]
async fn session_distill_marks_mentions_stale_and_replaces_pending_on_resave() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    db.query(
        "CREATE session:`s1` SET campaign=campaign:`camp1`, title='One', notes='Mira did things',
             session_number=1, date_played=NULL, created_at=time::now(), updated_at=time::now()",
    ).await.unwrap().check().unwrap();
    let llm: Arc<dyn chronacle_core::llm::LlmProvider> = Arc::new(MockLlm::with_response(
        r#"{"proposals":[{"kind":"entity_article_update","target_name":"Mira",
            "proposed_text":"Updated.","rationale":"r"}],"mentioned":["Mira"]}"#,
    ));
    let n1 = distill_session_notes(&db, &llm, "s1").await.unwrap();
    assert_eq!(n1, 1);
    let n2 = distill_session_notes(&db, &llm, "s1").await.unwrap();
    assert_eq!(n2, 1);
    assert_eq!(list_proposals(&db, Some("pending")).await.unwrap().len(), 1,
        "re-save replaces, never duplicates");

    #[derive(serde::Deserialize)]
    struct Npc { codex_stale: Option<bool> }
    let mut r = db.query("SELECT codex_stale FROM npc:`mira`").await.unwrap();
    let npc: Option<Npc> = r.take(0).unwrap();
    assert_eq!(npc.unwrap().codex_stale, Some(true));
}
```

> Adjust the `session` CREATE fields to the actual `session` schema in `001_base_schema.surql` if they differ (check field names before running).

Run: `cargo test -p chronacle-extraction proposals -- --nocapture` → FAIL (functions missing).

- [ ] **Step 2: Implement list/accept/reject/counts** (append to `proposals.rs`)

```rust
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
        Some(_) => "SELECT id, kind, target, payload, origin, status, created_at \
                    FROM codex_proposal WHERE status = $status ORDER BY created_at DESC",
        None => "SELECT id, kind, target, payload, origin, status, created_at \
                 FROM codex_proposal ORDER BY created_at DESC",
    };
    let mut q = db.query(sql);
    if let Some(s) = status {
        q = q.bind(("status", s.to_owned()));
    }
    let mut resp = q.await.map_err(|e| format!("Failed to list proposals: {e}"))?;
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
                struct Enrich { name: Option<String>, current: Option<String> }
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
                let e = e.unwrap_or(Enrich { name: None, current: None });
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
```

Accept/reject:

```rust
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
            let kind_str = row.payload.entity_kind.clone().unwrap_or_else(|| "misc".into());
            let kind = crate::entity_service::EntityKind::from_table(&kind_str)
                .unwrap_or(crate::entity_service::EntityKind::Misc);
            // Proposal.collection is required by the schema, so read it back.
            #[derive(Deserialize)]
            struct ColRow { collection: Thing }
            let mut cr = db
                .query("SELECT collection FROM type::thing('codex_proposal', $id)")
                .bind(("id", proposal_id.to_owned()))
                .await
                .map_err(|e| format!("Failed to read proposal collection: {e}"))?;
            let col: Option<ColRow> = cr
                .take(0)
                .map_err(|e| format!("Failed to parse proposal collection: {e}"))?;
            let col = col.ok_or("proposal has no collection")?.collection.id.to_raw();
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
            let name = row.payload.name.clone().ok_or("new_rule_entry needs a name")?;
            let category = row
                .payload
                .category
                .clone()
                .filter(|c| super::RULE_CATEGORIES.contains(&c.as_str()))
                .unwrap_or_else(|| "entry".into());
            #[derive(Deserialize)]
            struct ColRow { collection: Thing }
            let mut cr = db
                .query("SELECT collection FROM type::thing('codex_proposal', $id)")
                .bind(("id", proposal_id.to_owned()))
                .await
                .map_err(|e| format!("Failed to read proposal collection: {e}"))?;
            let col: Option<ColRow> = cr
                .take(0)
                .map_err(|e| format!("Failed to parse proposal collection: {e}"))?;
            let col = col.ok_or("proposal has no collection")?.collection.id.to_raw();
            #[derive(Deserialize)]
            struct IdRow(Thing);
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
    struct Counts { proposals: usize, findings: usize }
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
    let kind = crate::entity_service::EntityKind::from_table(table)
        .map_err(|e| e.to_string())?;
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
    struct Row { name: String, category: String, body: String }
    let mut resp = db
        .query("SELECT name, category, body FROM type::thing('rule_entry', $id)")
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| format!("Failed to load rule entry: {e}"))?;
    let row: Option<Row> = resp
        .take(0)
        .map_err(|e| format!("Failed to parse rule entry: {e}"))?;
    let Some(row) = row else { return Ok(()) };
    super::rules::embed_rule_entry(db, embed, id, &row.name, &row.category, &row.body)
        .await
        .map_err(|e| e.to_string())
}
```

In `rules.rs`, change `async fn embed_rule_entry` to `pub(super) async fn embed_rule_entry`. In `mod.rs`, `RULE_CATEGORIES` is already `pub(crate)` — usable via `super::RULE_CATEGORIES`.

- [ ] **Step 3: Run all proposal tests**

Run: `cargo test -p chronacle-extraction proposals -- --nocapture` → PASS (all 8).

- [ ] **Step 4: Commit**

```bash
git add crates/chronacle-extraction
git commit -m "feat(codex): proposal accept/reject with provenance and re-embed"
```

### Task 8: Tauri commands + session-save hook + PR

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands/codex_commands.rs`
- Modify: `apps/desktop/src-tauri/src/commands/session_commands.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs` (handler registration)
- Modify: `docs/architecture.md` (data-model + write-back paragraph in ADR-009 section)

**Interfaces:**
- Produces commands: `save_chat_to_codex(campaignId, content) -> usize`, `get_proposals(status?) -> Vec<CodexProposal>`, `accept_proposal(id)`, `reject_proposal(id)`, `get_maintenance_counts() -> MaintenanceCounts`. Session create/update fire a best-effort background distill.

- [ ] **Step 1: Add the commands** (append to `codex_commands.rs`)

```rust
/// Distill an assistant chat answer into pending codex proposals
/// ("Save to Codex"). Returns how many proposals were created.
#[tauri::command]
pub async fn save_chat_to_codex(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
    content: String,
) -> Result<usize, String> {
    let state_ref = state.inner().clone();
    let llm = state_ref
        .llm_provider
        .read()
        .map_err(|e| format!("LLM lock: {e}"))?
        .clone();
    chronacle_extraction::codex_service::distill_chat_answer(
        &state_ref.db,
        &llm,
        &campaign_id,
        &content,
    )
    .await
    .map_err(|e| e.to_string())
}

/// List codex proposals, optionally filtered by status ('pending' etc.).
#[tauri::command]
pub async fn get_proposals(
    state: State<'_, Arc<AppState>>,
    status: Option<String>,
) -> Result<Vec<chronacle_extraction::codex_service::CodexProposal>, String> {
    chronacle_extraction::codex_service::list_proposals(&state.db, status.as_deref()).await
}

/// Accept a proposal: apply it, append provenance, re-embed, resolve.
#[tauri::command]
pub async fn accept_proposal(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    let state_ref = state.inner().clone();
    let embed = state_ref
        .embedding_provider
        .read()
        .map_err(|e| format!("Embed lock: {e}"))?
        .clone();
    chronacle_extraction::codex_service::accept_proposal(&state_ref.db, &embed, &id).await
}

/// Reject a proposal without applying it.
#[tauri::command]
pub async fn reject_proposal(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    chronacle_extraction::codex_service::reject_proposal(&state.db, &id).await
}

/// Pending proposals + unresolved lint findings (sidebar badge).
#[tauri::command]
pub async fn get_maintenance_counts(
    state: State<'_, Arc<AppState>>,
) -> Result<chronacle_extraction::codex_service::MaintenanceCounts, String> {
    chronacle_extraction::codex_service::maintenance_counts(&state.db).await
}
```

- [ ] **Step 2: Session-save hook** (`session_commands.rs`) — after `embed_after_save` in both `create_session` and `update_session`:

```rust
/// Fire the C1 session-notes distillation in the background — best-effort:
/// the save must never fail or block on the LLM.
fn distill_after_save(state: &Arc<AppState>, session: &Session) {
    if session.notes.trim().is_empty() {
        return;
    }
    let llm = match state.llm_provider.read() {
        Ok(guard) => guard.clone(),
        Err(e) => {
            eprintln!("session distill: provider lock poisoned: {e}");
            return;
        }
    };
    let db = state.db.clone();
    let session_id = session.id.clone();
    tokio::spawn(async move {
        match chronacle_extraction::codex_service::distill_session_notes(&db, &llm, &session_id)
            .await
        {
            Ok(n) if n > 0 => eprintln!("session distill: {n} proposal(s) created"),
            Ok(_) => {}
            Err(e) => eprintln!("session distill failed for {session_id}: {e}"),
        }
    });
}
```

Call `distill_after_save(state.inner(), &session);` in `create_session` and `update_session` before returning (`State<'_, Arc<AppState>>::inner()` yields the `&Arc<AppState>` the helper takes).

- [ ] **Step 3: Register handlers** (`lib.rs` `generate_handler!` list):

```rust
            commands::save_chat_to_codex,
            commands::get_proposals,
            commands::accept_proposal,
            commands::reject_proposal,
            commands::get_maintenance_counts,
```

- [ ] **Step 4: Docs, full gate, push, stacked PR**

`docs/architecture.md`: extend the ADR-009 section with one paragraph on the write-back path (producers → `codex_proposal` → explicit accept; notes-update is the sole machine→user-field path).

```bash
cargo fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace
pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend
git add -A && git commit -m "feat(commands): codex proposal commands + session distill hook"
git push -u origin feat/c1a-proposals-backend:refs/heads/feat/c1a-proposals-backend
gh pr create --base feat/b3b-codex-retrieval --head feat/c1a-proposals-backend --title "feat(codex): write-back proposal producers and review service (C1a)" --body "..."
```

---

# PR C1b — `feat/c1b-inbox-ui`

Maintenance inbox (Proposals tab), Save-to-Codex chat action, sidebar badge.

Branch: `git checkout --no-track -b feat/c1b-inbox-ui feat/c1a-proposals-backend`

### Task 9: Invoke wrappers + MaintenanceView (Proposals tab)

**Files:**
- Modify: `apps/desktop/src/lib/commands.ts` (types + wrappers from Shared interfaces)
- Create: `apps/desktop/src/views/MaintenanceView.svelte`
- Create: `apps/desktop/src/views/MaintenanceView.test.ts`

**Interfaces:**
- Consumes: `get_proposals`, `accept_proposal`, `reject_proposal`, `get_maintenance_counts` commands (C1a).
- Produces: `MaintenanceView` with props `{ onCountsChanged: () => void }`; exported invoke wrappers per Shared interfaces.

- [ ] **Step 1: Add commands.ts types + wrappers** — copy the TS block from "Shared interfaces" into a new `// ── Maintenance ──` section:

```ts
/** Distill an assistant answer into pending codex proposals; returns the count created. */
export async function saveChatToCodex(campaignId: string, content: string): Promise<number> {
  return invoke<number>('save_chat_to_codex', { campaignId, content });
}

/** List codex proposals, optionally filtered by status ('pending', 'accepted', 'rejected'). */
export async function getProposals(status?: string): Promise<CodexProposal[]> {
  return invoke<CodexProposal[]>('get_proposals', { status: status ?? null });
}

/** Accept a proposal: applies the change, appends provenance, re-embeds. */
export async function acceptProposal(id: string): Promise<void> {
  return invoke('accept_proposal', { id });
}

/** Reject a proposal without applying it. */
export async function rejectProposal(id: string): Promise<void> {
  return invoke('reject_proposal', { id });
}

/** Pending proposals + unresolved lint findings, for the Maintenance badge. */
export async function getMaintenanceCounts(): Promise<MaintenanceCounts> {
  return invoke<MaintenanceCounts>('get_maintenance_counts');
}
```

(with the `ProposalPayload` / `CodexProposal` / `MaintenanceCounts` interfaces above them.)

- [ ] **Step 2: Write failing Vitest tests** (`MaintenanceView.test.ts`, mock `@tauri-apps/api/core` like sibling view tests do)

```ts
// Scenarios:
// 1. renders pending proposals with kind label, target name, rationale
// 2. shows current vs proposed text side-by-side (both texts in the document)
// 3. Accept button invokes accept_proposal with the row id, then refetches
// 4. Reject button invokes reject_proposal with the row id
// 5. empty state renders "No pending proposals"
```

Write them fully, following the mocking pattern of `RulesPanel.test.ts` (mock `invoke`, render with `@testing-library/svelte`, `await findByText`, click, assert `invoke` calls).

Run: `pnpm -C apps/desktop test:run -- MaintenanceView` → FAIL (component missing).

- [ ] **Step 3: Implement `MaintenanceView.svelte`**

Structure (Svelte 5 runes; styles follow existing view conventions — reuse token variables like `var(--bg-panel)`, `var(--line)`):

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from '../components/Icon.svelte';
  import {
    getProposals, acceptProposal, rejectProposal,
    type CodexProposal,
  } from '../lib/commands';

  let { onCountsChanged = () => {} }: { onCountsChanged?: () => void } = $props();

  let proposals = $state<CodexProposal[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state<string | null>(null); // proposal id being resolved

  const KIND_LABELS: Record<string, string> = {
    entity_article_update: 'Article update',
    entity_notes_update: 'Notes suggestion',
    rule_entry_update: 'Rule update',
    new_entity: 'New entity',
    new_rule_entry: 'New rule',
  };

  async function refresh() {
    loading = true;
    error = null;
    try {
      proposals = await getProposals('pending');
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function resolve(id: string, action: 'accept' | 'reject') {
    busy = id;
    try {
      if (action === 'accept') await acceptProposal(id);
      else await rejectProposal(id);
      await refresh();
      onCountsChanged();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  onMount(() => void refresh());
</script>
```

Markup: header "Maintenance", tab strip with a single `Proposals` tab (`role="tablist"`, prepared for C2b's second tab), then a list of proposal cards:

- kind chip (`KIND_LABELS[p.kind] ?? p.kind`), target name (`p.target_name ?? p.payload.name`), origin chip (`p.origin_kind`)
- rationale paragraph
- diff: two `pre-wrap` panes side by side — left "Current" (`p.current_text ?? '(none)'`), right "Proposed" (`p.payload.proposed_text`)
- Accept / Reject buttons (`disabled={busy === p.id}`, `aria-label` "Accept proposal" / "Reject proposal")
- empty state: "No pending proposals".

- [ ] **Step 4: Run tests, commit**

Run: `pnpm -C apps/desktop test:run -- MaintenanceView` → PASS.

```bash
git add apps/desktop/src
git commit -m "feat(ui): maintenance inbox with proposal diff review"
```

### Task 10: Sidebar item + badge, Save-to-Codex chat action

**Files:**
- Modify: `apps/desktop/src/shell/CampaignRail.svelte` (View union + nav item + badge)
- Modify: `apps/desktop/src/shell/Shell.svelte` (render view, counts polling, topbar title)
- Modify: `apps/desktop/src/views/OracleView.svelte` (Save to Codex on assistant messages)
- Test: `apps/desktop/src/shell/Shell.test.ts`, `apps/desktop/src/views/OracleView.test.ts`

**Interfaces:**
- Consumes: `MaintenanceView` (Task 9), `saveChatToCodex`, `getMaintenanceCounts`.
- Produces: `View` union gains `'maintenance'`; `CampaignRail` prop `maintenanceCount: number`; `OracleView` props gain `onSavedToCodex?: (count: number) => void`.

- [ ] **Step 1: Write failing tests**

`Shell.test.ts` additions: "rail shows Maintenance item with badge when counts are non-zero" and "clicking Maintenance renders MaintenanceView" (mock `get_maintenance_counts` → `{ pending_proposals: 3, unresolved_findings: 1 }`, assert badge text `4`).

`OracleView.test.ts` additions: "assistant message shows Save to Codex action" and "clicking it invokes save_chat_to_codex with campaign id and message content, then shows a toast" (mock resolved value `2`, assert toast text contains "2 proposal").

Run: `pnpm -C apps/desktop test:run -- Shell OracleView` → FAIL.

- [ ] **Step 2: Implement**

`CampaignRail.svelte`:
- `export type View = 'oracle' | 'campaign' | 'settings' | 'timeline' | 'maintenance' | { kind: 'notebook'; category: NoteCategoryId };`
- new prop `maintenanceCount = 0`
- nav item after Timeline:

```svelte
    <button
      class="nav-item"
      class:active={view === 'maintenance'}
      onclick={() => setView('maintenance')}
    >
      <Icon name="inbox" size={18} className="ic" />
      Maintenance
      {#if maintenanceCount > 0}
        <span class="ct badge">{maintenanceCount}</span>
      {/if}
    </button>
```

(add a `.ct.badge` style: pill background `rgba(91,120,255,0.25)`, `border-radius: var(--r-full)`, `padding: 1px 7px`. If icon `inbox` is missing from `Icon.svelte`, use an existing one, e.g. `library`, or add the `inbox` path following the component's existing icon-map pattern.)

`Shell.svelte`:
- `let maintenanceCount = $state(0);` + `async function refreshMaintenanceCount()` calling `getMaintenanceCounts()` and summing; call it in the existing `onMount` bootstrap and after inbox actions.
- pass `maintenanceCount={maintenanceCount}` to `CampaignRail`.
- topbar title branch: `if (view === 'maintenance') return { title: 'Maintenance', sub: 'Codex proposals and lint findings' };`
- view branch:

```svelte
    {:else if view === 'maintenance'}
      <MaintenanceView onCountsChanged={refreshMaintenanceCount} />
```

- pass `onSavedToCodex={(n) => { refreshMaintenanceCount(); }}` to `OracleView` if the toast lives in Shell — otherwise OracleView toasts itself and just calls the callback.

`OracleView.svelte`:
- new optional prop `onSavedToCodex`
- under each assistant message (both the `RulingCard` branch and the plain-assistant branch), an action row:

```svelte
        <div class="msg-actions">
          <button
            class="save-codex"
            disabled={savingToCodex === i}
            onclick={() => saveToCodex(i, msg.content)}
            title="Distill this answer into codex proposals"
          >
            <Icon name="book-plus" size={13} /> Save to Codex
          </button>
        </div>
```

with:

```ts
  let savingToCodex = $state<number | null>(null);

  async function saveToCodex(index: number, content: string) {
    if (!activeCampaignId) {
      messages = [...messages, { role: 'error', content: 'Select a campaign first.' }];
      return;
    }
    savingToCodex = index;
    try {
      const n = await saveChatToCodex(activeCampaignId, content);
      toast(n === 0 ? 'Nothing worth saving found.' : `${n} proposal${n === 1 ? '' : 's'} created — review in Maintenance.`);
      onSavedToCodex?.(n);
    } catch (e) {
      toast(`Save to Codex failed: ${String(e)}`);
    } finally {
      savingToCodex = null;
    }
  }
```

(match the view's existing toast mechanism — check how OracleView surfaces transient notices; if it has none, use the shared `Toast.svelte` component the way `CampaignView` does. `activeCampaignId` — reuse the prop OracleView already receives for `chatSend`.)

- [ ] **Step 3: Run tests, then the whole frontend suite**

Run: `pnpm -C apps/desktop test:run` → PASS.

```bash
git add apps/desktop/src
git commit -m "feat(ui): maintenance badge and save-to-codex chat action"
```

### Task 11: C1 acceptance feature + user guide + PR

**Files:**
- Create: `apps/desktop/tests/e2e/features/maintenance-inbox.feature`
- Create/modify: `apps/desktop/tests/e2e/backend/steps/maintenance.steps.ts`
- Modify: `docs/user-guide.md` ("The Codex" chapter: Save to Codex + inbox section)

- [ ] **Step 1: Write the feature** (verbatim-tightened from the spec's C1 scenarios)

```gherkin
Feature: Codex write-back review
  Durable results reach the codex only through reviewed proposals (ADR-009).

  Background:
    Given the app is running with a seeded campaign "Test Campaign"
    When the GM opens the app

  Scenario: Saving an answer to the codex creates reviewable proposals
    Given the assistant has answered a question
    When the GM clicks "Save to Codex" on the answer
    Then the save-to-codex command is sent with the answer text
    And a toast reports the created proposals

  Scenario: Accepting a proposal applies it and rejecting changes nothing
    Given the maintenance inbox lists a pending proposal for "Mira"
    When the GM accepts the proposal
    Then the accept command is sent for that proposal
    When the GM rejects the remaining proposal
    Then the reject command is sent for that proposal
```

- [ ] **Step 2: Write `maintenance.steps.ts`** following `codex.steps.ts` conventions — `installIpcMock` with `get_chat_history` (one assistant message), `save_chat_to_codex: 2`, `get_proposals: [<two pending fixtures>]`, `get_maintenance_counts`, `accept_proposal: null`, `reject_proposal: null`; assert `__ipcCalls` contains the expected `cmd` + args.

- [ ] **Step 3: User guide** — extend the "The Codex" chapter with a "Saving answers and session notes" section (what Save to Codex does, that nothing changes until accepted, where the inbox lives) in GM language.

- [ ] **Step 4: Full gate, push, stacked PR**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace
pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend
git add -A && git commit -m "test(e2e): maintenance inbox acceptance; codex guide update"
git push -u origin feat/c1b-inbox-ui:refs/heads/feat/c1b-inbox-ui
gh pr create --base feat/c1a-proposals-backend --head feat/c1b-inbox-ui --title "feat(ui): maintenance inbox and save-to-codex (C1b)" --body "..."
```

---

# PR C2a — `feat/c2a-lint-pass`

Pure-Rust lint detectors (scope, wikilink, stale, duplicate), manual pass, list/resolve service + commands.

Branch: `git checkout --no-track -b feat/c2a-lint-pass feat/c1b-inbox-ui`

### Task 12: Lint detectors + pass

**Files:**
- Create: `crates/chronacle-extraction/src/codex_service/lint.rs`
- Create: `crates/chronacle-extraction/src/codex_service/lint_tests.rs`
- Modify: `crates/chronacle-extraction/src/codex_service/mod.rs` (register + re-export)
- Modify: `crates/chronacle-extraction/src/entity_service/relations/mod.rs` + `scope.rs` (widen `check_scope` to `pub(crate)`)

**Interfaces:**
- Consumes: `record_lint` (mod.rs), `query_all_entity_names` (`pub(crate)` since C1a), `entity_service::relations::scope::check_scope` (widen to `pub(crate)`), `WIKILINK_RE`-equivalent parsing (local copy — the wikilink regex is 1 line; duplicate it rather than widening more internals).
- Produces: `run_lint_campaign`, `run_lint_collection`, `list_lint_findings`, `resolve_lint_finding`, `LintSummary`, `LintFinding` (see Shared interfaces).

- [ ] **Step 1: Widen `check_scope`** — in `relations/scope.rs` change `pub(super) async fn check_scope` to `pub(crate) async fn check_scope`; in `relations/mod.rs` add `pub(crate) use scope::check_scope;` (and re-export from `entity_service/mod.rs` as `pub(crate) use relations::check_scope;` if the module tree needs it).

- [ ] **Step 2: Write failing tests** (`lint_tests.rs`, registered in `mod.rs` under `#[cfg(test)]`) — one test per detector plus dedup + resolve, using the same `setup_db`/`seed_campaign` helpers as `proposals_tests.rs` (extract shared helpers into a `#[cfg(test)] mod test_util` in `codex_service/mod.rs` if duplication itches):

```rust
#[tokio::test]
async fn broken_wikilink_is_found_and_clears_when_entity_exists() {
    // npc `mira` in own1 with notes = "See [[Nonexistent]] and [[Mira]]".
    // run_lint_campaign → exactly one broken_wikilink finding, payload.link_text
    // = "Nonexistent" ([[Mira]] resolves). Create npc named Nonexistent in own1,
    // run again → no NEW broken_wikilink finding for it (existing one still
    // listed until resolved — the resolve action is the user's).
}

#[tokio::test]
async fn duplicate_entity_flags_same_named_pairs_in_scope() {
    // Two npcs named "Korim" (case-differing: "Korim"/"korim") in own1
    // → one duplicate_entity finding with payload.a, payload.b, similarity 1.0.
}

#[tokio::test]
async fn stale_article_aggregates_needs_compile_entities() {
    // npc with codex_stale=true → stale_article finding with payload.reason.
}

#[tokio::test]
async fn scope_violation_found_for_pre_enforcement_edge() {
    // Two regular collections ca/cb, entity in each, a relates_to edge created
    // RAW via db.query (bypassing entity_service validation, simulating legacy
    // data) → scope_violation finding with from/to payload.
}

#[tokio::test]
async fn lint_pass_is_idempotent_no_duplicate_findings() {
    // Run run_lint_campaign twice on the broken-wikilink fixture; the second
    // run creates 0 new findings (summary.new_findings == 0).
}

#[tokio::test]
async fn resolve_lint_finding_sets_resolved_at() {
    // list → resolve first id → unresolved count drops by one.
}
```

Write each fully with real seeding SQL (reuse the `seed_campaign` idiom). Run: `cargo test -p chronacle-extraction lint -- --nocapture` → FAIL.

- [ ] **Step 3: Implement `lint.rs`**

```rust
//! Manual lint pass (ADR-009 C2): pure-Rust detectors that surface data drift
//! as `lint_finding` rows for the Maintenance inbox. No LLM calls here —
//! contradiction detection is explicitly deferred.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use surrealdb::Connection;

use super::record_lint;
use crate::wikilink::{query_all_entity_names, WikilinkScope};

static WIKILINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\[\]]+)\]\]").expect("wikilink regex is valid"));

/// Result of one lint pass.
#[derive(Debug, Clone, Serialize)]
pub struct LintSummary {
    pub new_findings: usize,
    pub unresolved_total: usize,
}

/// One unresolved finding (payload is kind-shaped; read-only Value is safe).
#[derive(Debug, Clone, Serialize)]
pub struct LintFinding {
    pub id: String,
    pub kind: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}
```

Core helpers:

```rust
/// True when an unresolved finding of `kind` whose payload field `key`
/// equals `value` already exists (idempotent re-runs).
async fn finding_exists<C: Connection>(
    db: &surrealdb::Surreal<C>,
    kind: &str,
    key: &str,
    value: &str,
) -> Result<bool, String> {
    #[derive(Deserialize)]
    struct CountRow { count: i64 }
    let mut resp = db
        .query(format!(
            "SELECT count() FROM lint_finding WHERE kind = $kind \
                 AND resolved_at = NONE AND payload.`{key}` = $val GROUP ALL"
        ))
        .bind(("kind", kind.to_owned()))
        .bind(("val", value.to_owned()))
        .await
        .map_err(|e| format!("Failed lint dedup check: {e}"))?;
    let rows: Vec<CountRow> = resp.take(0).map_err(|e| format!("Failed lint dedup parse: {e}"))?;
    Ok(rows.first().map(|r| r.count).unwrap_or(0) > 0)
}
```

Detectors — each takes `db` + the in-scope entity list `&[(String /*full_id*/, String /*name*/)]` and returns `Result<usize, String>` (findings created):

1. **`lint_broken_wikilinks`** — fetch `notes` and `codex_article` for every in-scope entity (one query per table with the ids, or reuse `wikilink::query_all_entity_notes` by also widening it `pub(crate)`; articles need a separate `SELECT id, codex_article FROM {table} WHERE id IN […]` — build the id list inline, no KNN involved so `IN (SELECT …)` composition worries don't apply, but inline arrays are still simplest). For each `[[link]]` whose trimmed lowercase text matches no in-scope name → `record_lint(db, "broken_wikilink", json!({ "entity": full_id, "link_text": link }))` unless `finding_exists("broken_wikilink", "link_text", link)` **and** same entity — dedup key: check `payload.entity` and `payload.link_text` (extend `finding_exists` with a second key or write the two-key variant inline).
2. **`lint_duplicates`** — group in-scope entities by `(table, name.trim().to_lowercase())`; for each group ≥ 2, emit pairwise findings `json!({ "a": id_a, "b": id_b, "similarity": 1.0 })`, dedup on `payload.a` + `payload.b`.
3. **`lint_stale_articles`** — for in-scope entities, `SELECT id FROM {table} WHERE id IN […] AND (codex_stale != false OR codex_article = NONE)` → `json!({ "entity": id, "reason": "stale or uncompiled" })`, dedup on `payload.entity`.
4. **`lint_scope_violations`** — `SELECT id, in, out FROM relates_to WHERE in IN […] OR out IN […]` (inline id array of in-scope entities); for each edge call `crate::entity_service::check_scope(db, in_tb, in_id, out_tb, out_id)`; on `Err(EntityError::ScopeViolation {..})` → `json!({ "edge": edge_id, "from": …, "to": … })`, dedup on `payload.edge`.

Pass entry points:

```rust
/// Run every detector over a campaign's full scope (own + subscribed).
pub async fn run_lint_campaign<C: Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
) -> Result<LintSummary, String> {
    let entities = query_all_entity_names(db, &WikilinkScope::Campaign { campaign_id })
        .await
        .map_err(|e| e.to_string())?;
    run_detectors(db, &entities).await
}

/// Run every detector over a single collection's scope.
pub async fn run_lint_collection<C: Connection>(
    db: &surrealdb::Surreal<C>,
    collection_id: &str,
) -> Result<LintSummary, String> {
    let entities = query_all_entity_names(db, &WikilinkScope::Collection { collection_id })
        .await
        .map_err(|e| e.to_string())?;
    run_detectors(db, &entities).await
}

async fn run_detectors<C: Connection>(
    db: &surrealdb::Surreal<C>,
    entities: &[(String, String)],
) -> Result<LintSummary, String> {
    let mut new_findings = 0;
    new_findings += lint_broken_wikilinks(db, entities).await?;
    new_findings += lint_duplicates(db, entities).await?;
    new_findings += lint_stale_articles(db, entities).await?;
    new_findings += lint_scope_violations(db, entities).await?;
    let unresolved_total = unresolved_count(db).await?;
    Ok(LintSummary { new_findings, unresolved_total })
}
```

List/resolve:

```rust
/// All unresolved findings, newest first.
pub async fn list_lint_findings<C: Connection>(
    db: &surrealdb::Surreal<C>,
) -> Result<Vec<LintFinding>, String> {
    #[derive(Deserialize)]
    struct Row { id: Thing, kind: String, payload: serde_json::Value,
                 created_at: surrealdb::sql::Datetime }
    let mut resp = db
        .query(
            "SELECT id, kind, payload, created_at FROM lint_finding \
             WHERE resolved_at = NONE ORDER BY created_at DESC",
        )
        .await
        .map_err(|e| format!("Failed to list findings: {e}"))?;
    let rows: Vec<Row> = resp.take(0).map_err(|e| format!("Failed to parse findings: {e}"))?;
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
        .map_err(|e| format!("Failed to resolve finding: {e}"))?;
    Ok(())
}
```

Register + re-export in `codex_service/mod.rs`:

```rust
mod lint;
#[cfg(test)]
mod lint_tests;
pub use lint::{
    list_lint_findings, resolve_lint_finding, run_lint_campaign, run_lint_collection,
    LintFinding, LintSummary,
};
```

- [ ] **Step 4: Run tests, commit**

Run: `cargo test -p chronacle-extraction lint -- --nocapture` → PASS.

```bash
git add crates/chronacle-extraction
git commit -m "feat(codex): pure-rust lint detectors and manual pass"
```

### Task 13: Lint commands + delete-relation + PR

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands/codex_commands.rs`
- Modify: `apps/desktop/src-tauri/src/commands/entity_commands.rs` (`delete_relation`)
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `crates/chronacle-db/src/schema/002_wiki_layer.surql` (comment block only: mark C2 kinds as produced)

- [ ] **Step 1: Commands**

```rust
/// Run the manual lint pass over a campaign's full scope ("Check campaign").
#[tauri::command]
pub async fn run_lint(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
) -> Result<chronacle_extraction::codex_service::LintSummary, String> {
    chronacle_extraction::codex_service::run_lint_campaign(&state.db, &campaign_id).await
}

/// Unresolved lint findings for the Maintenance inbox.
#[tauri::command]
pub async fn get_lint_findings(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<chronacle_extraction::codex_service::LintFinding>, String> {
    chronacle_extraction::codex_service::list_lint_findings(&state.db).await
}

/// Mark one lint finding resolved.
#[tauri::command]
pub async fn resolve_lint_finding(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    chronacle_extraction::codex_service::resolve_lint_finding(&state.db, &id).await
}
```

`entity_commands.rs` — `delete_relation` (used by the scope-violation resolve action):

```rust
/// Delete one `relates_to` edge by its record id (Maintenance resolve action).
#[tauri::command]
pub async fn delete_relation(
    state: State<'_, Arc<AppState>>,
    edge_id: String,
) -> Result<(), String> {
    state
        .db
        .query("DELETE type::thing('relates_to', $id)")
        .bind(("id", edge_id))
        .await
        .map_err(|e| format!("Failed to delete relation: {e}"))?;
    Ok(())
}
```

(plus a Rust test in the same file: create an edge raw, delete via the service body logic, assert count 0 — follow the module's existing test style.)

Register all four in `lib.rs`.

- [ ] **Step 2: Schema comment** — in `002_wiki_layer.surql`'s lint-kind block, update the C2 annotations from "(C2)" to note the producing pass (comment-only change; **no** DEFINE changes).

- [ ] **Step 3: Full gate, push, stacked PR**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace
pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend
git add -A && git commit -m "feat(commands): lint pass, findings, resolve, delete-relation"
git push -u origin feat/c2a-lint-pass:refs/heads/feat/c2a-lint-pass
gh pr create --base feat/c1b-inbox-ui --head feat/c2a-lint-pass --title "feat(codex): lint detectors and manual pass (C2a)" --body "..."
```

---

# PR C2b — `feat/c2b-lint-ui`

Maintenance inbox Findings tab with per-kind resolve actions; "Check campaign" button.

Branch: `git checkout --no-track -b feat/c2b-lint-ui feat/c2a-lint-pass`

### Task 14: Findings tab + resolve actions

**Files:**
- Modify: `apps/desktop/src/lib/commands.ts` (`runLint`, `getLintFindings`, `resolveLintFinding`, `deleteRelation` + `LintFinding`/`LintSummary` types)
- Modify: `apps/desktop/src/views/MaintenanceView.svelte`
- Modify: `apps/desktop/src/views/MaintenanceView.test.ts`
- Modify: `apps/desktop/src/shell/Shell.svelte` (pass `onOpenEntity` down so findings can open entities)

**Interfaces:**
- Consumes: C2a commands; `openEntity(id, kind)` plumbing already used by `TimelineView` (`onOpenEntity` prop pattern).
- Produces: `MaintenanceView` props become `{ onCountsChanged?, onOpenEntity?: (id: string, kind: string) => void }`.

- [ ] **Step 1: commands.ts wrappers** (per Shared interfaces, same doc-comment style as Task 9).

- [ ] **Step 2: Write failing Vitest tests** (extend `MaintenanceView.test.ts`):

```ts
// 6. Findings tab lists findings grouped by kind with human labels
// 7. "Check campaign" button invokes run_lint and shows the summary
// 8. broken_wikilink finding has "Open entity" (calls onOpenEntity with parsed
//    table+id) and "Mark resolved" (invokes resolve_lint_finding)
// 9. stale_article finding has "Compile" (invokes compile_entity) then resolves
// 10. scope_violation finding has "Delete edge" (invokes delete_relation) then resolves
// 11. duplicate_entity finding has two "Open" actions (a and b)
```

Run: `pnpm -C apps/desktop test:run -- MaintenanceView` → FAIL.

- [ ] **Step 3: Implement**

`MaintenanceView.svelte` additions:
- `let tab = $state<'proposals' | 'findings'>('proposals');` — real tab strip (`role="tab"`, `aria-selected`).
- `let findings = $state<LintFinding[]>([]);` loaded in `refresh()` alongside proposals; group with `$derived` by `kind`.
- `activeCampaignId` prop (from Shell) + "Check campaign" button in the header:

```ts
  async function checkCampaign() {
    if (!activeCampaignId) return;
    checking = true;
    try {
      const s = await runLint(activeCampaignId);
      lintNote = `${s.new_findings} new finding${s.new_findings === 1 ? '' : 's'} · ${s.unresolved_total} open`;
      await refresh();
      onCountsChanged();
    } catch (e) {
      error = String(e);
    } finally {
      checking = false;
    }
  }
```

- Kind labels + per-kind action rows (payload fields per the schema comment block):

```ts
  const FINDING_LABELS: Record<string, string> = {
    orphaned_edge: 'Orphaned edge',
    scope_violation: 'Scope violation',
    broken_wikilink: 'Broken wikilink',
    stale_article: 'Stale article',
    duplicate_entity: 'Possible duplicate',
  };

  function entityRef(v: unknown): { id: string; kind: string } | null {
    if (typeof v !== 'string' || !v.includes(':')) return null;
    const [kind, id] = v.split(':', 2);
    return { id, kind };
  }
```

Actions per kind (each also gets "Mark resolved" → `resolveLintFinding(f.id)` then `refresh()` + `onCountsChanged()`):
  - `broken_wikilink`: show `payload.link_text`; "Open entity" → `onOpenEntity?.(ref.id, ref.kind)` for `payload.entity`.
  - `stale_article`: "Compile" → `compileEntity(ref.kind, ref.id)` then auto-resolve the finding.
  - `scope_violation`: show from/to; "Delete edge" → `deleteRelation(String(payload.edge))` then auto-resolve.
  - `duplicate_entity`: "Open A" / "Open B" via `onOpenEntity` (entity merge deferred — spec open question №1 resolved as link-to-both).
  - `orphaned_edge`: details + "Mark resolved" only.

`Shell.svelte`: pass `activeCampaignId` and `onOpenEntity={(id, kind) => openEntity(id, kind)}` to `MaintenanceView` (match `openEntity`'s existing signature — check how `TimelineView` calls it and mirror).

- [ ] **Step 4: Run tests, commit**

Run: `pnpm -C apps/desktop test:run` → PASS.

```bash
git add apps/desktop/src
git commit -m "feat(ui): lint findings tab with per-kind resolve actions"
```

### Task 15: C2 acceptance feature + docs + PR

**Files:**
- Modify: `apps/desktop/tests/e2e/features/maintenance-inbox.feature` (findings scenarios)
- Modify: `apps/desktop/tests/e2e/backend/steps/maintenance.steps.ts`
- Modify: `docs/user-guide.md` ("The Codex" chapter: lint checks section)
- Modify: `docs/architecture.md` (ADR-009: C-series status note)

- [ ] **Step 1: Feature scenarios** (spec C2, tightened; the detector logic itself is covered by the Rust tests in Task 12 — these bind the UI):

```gherkin
  Scenario: A broken wikilink surfaces as a finding the GM can act on
    Given the maintenance inbox has a broken-wikilink finding for "[[Nonexistent]]"
    When the GM opens the findings tab
    Then the finding "Broken wikilink" is listed with "Nonexistent"
    When the GM marks the finding resolved
    Then the resolve command is sent for that finding

  Scenario: Same-named entities surface as a possible duplicate
    Given the maintenance inbox has a duplicate-entity finding for "Korim"
    When the GM opens the findings tab
    Then the finding "Possible duplicate" is listed with "Korim"
```

- [ ] **Step 2: Steps** — extend `maintenance.steps.ts` with `get_lint_findings` fixtures and `__ipcCalls` assertions, same pattern as Task 11.

- [ ] **Step 3: Docs** — user guide: "Keeping the codex healthy" section (what each check finds, what each resolve action does, in GM terms). Architecture doc: one status line on ADR-009 (B3+C series landed; contradiction detection deferred).

- [ ] **Step 4: Full gate, push, stacked PR**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace
pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend
git add -A && git commit -m "test(e2e): findings-tab acceptance; codex lint docs"
git push -u origin feat/c2b-lint-ui:refs/heads/feat/c2b-lint-ui
gh pr create --base feat/c2a-lint-pass --head feat/c2b-lint-ui --title "feat(ui): maintenance findings tab with resolve actions (C2b)" --body "..."
```

---

## Merge order & stack maintenance

Merge bottom-up: B3a → B3b → C1a → C1b → C2a → C2b, retargeting each PR's base to `main` as its parent merges, rebasing the remaining stack (`git rebase --onto main <old-parent> <branch>`), and force-pushing with `--force-with-lease`.

## Out of scope (deliberately)

- LLM-driven `contradiction` lint (spec: explicitly deferred).
- Entity merge for `duplicate_entity` (open question №1 → resolved: link to both, defer merge).
- Manual proposal drafting from entity/rule pages (spec's "small escape hatch") — the table and service support it; the UI affordance can ride any later PR.
- D-series vault sync.
