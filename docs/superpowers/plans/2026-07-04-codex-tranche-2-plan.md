# Codex Tranche 2 (B1a, B1b, B2a, B2b) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Codex compile pipelines: setting articles compiled onto entities (B1a backend, B1b UI) and rules compiled into `rule_entry` rows with redo-with-objections (B2a backend, B2b UI).

**Architecture:** Follows `docs/superpowers/specs/2026-07-03-codex-compiled-world-model-design.md` (sections "Compile pipelines", "UX"). Backend lives in `chronacle-extraction::codex_service` (extends the A2b skeleton), reusing extraction's `llm_complete`/`batch_passages`/`search_passages` idioms and the `extract-progress`-style abortable Tauri command pattern. UI extends `CampaignView`'s collection rows (Compile button + staleness badge + Books/Rules tab strip) and `EntityManager`'s form panel (read-only Codex Article + Recompile).

**Tech Stack:** Rust (SurrealDB embedded, tokio), Svelte 5 runes + TypeScript, Vitest, playwright-bdd.

## Global Constraints

- Every branch: `git checkout --no-track -b <branch> main` — never track main. B1b branches from B1a's branch if B1a is unmerged (stacked PR, base = B1a branch); same for B2a→B1a and B2b→B2a. Reconcile by rebasing after upstream merges (rebase-merge repo).
- No new crates, no new Cargo or npm dependencies anywhere in this tranche. Markdown rendering of articles is deliberately NOT added — articles render through the existing `WikiText` component (clickable `[[wikilinks]]`, plain text) inside a `white-space: pre-wrap` container.
- Commit subjects ≤ 72 chars, imperative, conventional prefixes; never `--no-verify`.
- Clippy warnings are errors; public items need `///` docs; Svelte 5 runes only.
- BDD (ADR-011): UI-reachable scenarios ship as `.feature` files (B1b, B2b); backend-only scenarios ship as Rust tests mirroring the Gherkin (B1a, B2a) per `apps/desktop/tests/e2e/features/README.md`.
- **Unset-staleness rule (A2a carry-forward):** rows created before the codex migration have `codex_stale` unset (NONE), not `false`. Every needs-compile query must treat unset as stale: `WHERE codex_stale != false OR codex_article = NONE` (SurrealDB: `NONE != false` is true).
- Opaque provenance objects (`codex_sources`, `rule_entry.sources`, `page_refs`) are FLEXIBLE — nested keys persist. Never bind a `serde_json::Value` where a plain struct works; structs and inline SurrealQL object literals are the proven-safe paths.
- Compile caps: `MAX_COMPILE_PER_RUN: usize = 50` entities and `MAX_RULE_BATCHES_PER_RUN: usize = 40` (constants in `codex_service`, mirroring `MAX_ENRICH`'s precedent; a capped run reports how many remain).
- LLM/embedding access only via `Arc<dyn LlmProvider>` / `Arc<dyn EmbeddingProvider>` / `Arc<dyn VectorStore>`; tests use `MockLlm`, `MockEmbeddingProvider`, `MockVectorStore` from `extraction_service::test_support` (make them `pub(crate)`-reachable from `codex_service` tests — they already are, same crate).
- Scoped provenance (ADR-009): compiling a **campaign-bound** collection searches the owner campaign's full subscription set; a **regular** collection searches only itself.
- Each PR ends green on: `cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace && pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend`.

## Shared interfaces introduced by this tranche (single source of truth)

```rust
// crates/chronacle-extraction/src/codex_service/ (module split below)
pub enum CodexPhase { Resolving, Compiling, Embedding, Done, Empty }        // serde snake_case
pub struct CompileProgress { pub phase: CodexPhase, pub detail: String,
                             pub compiled: usize, pub total: usize }
pub struct CompileResult { pub articles_compiled: usize, pub remaining_stale: usize }
pub struct RulesCompileResult { pub entries_created: usize, pub entries_updated: usize,
                                pub remaining_batches: usize }
pub struct CodexStatus { pub stale_entities: usize, pub total_entities: usize,
                         pub rules_stale: usize, pub rule_entries: usize }

pub async fn codex_status(db, collection_id) -> Result<CodexStatus, String>;                 // B1a
pub async fn compile_collection(db, llm, embed, vector_store, collection_id, on_progress)    // B1a
    -> Result<CompileResult, CodexError>;
pub async fn compile_entity(db, llm, embed, vector_store, table, id)                          // B1a
    -> Result<bool, CodexError>;   // false = no context found, article unchanged
pub async fn compile_rules(db, llm, embed, collection_id, on_progress)                        // B2a
    -> Result<RulesCompileResult, CodexError>;
pub async fn redo_rule_entry(db, llm, embed, rule_entry_id, objection)                        // B2a
    -> Result<(), CodexError>;
pub async fn list_rule_entries(db, collection_id) -> Result<Vec<RuleEntry>, String>;          // B2a
pub async fn update_rule_notes(db, rule_entry_id, notes: Option<String>) -> Result<(), String>; // B2a
```

```ts
// apps/desktop/src/lib/commands.ts additions
export type CodexPhase = 'resolving' | 'compiling' | 'embedding' | 'done' | 'empty';
export interface CompileProgress { phase: CodexPhase; detail: string; compiled: number; total: number; }
export interface CodexStatus { stale_entities: number; total_entities: number; rules_stale: number; rule_entries: number; }
export interface RuleEntry { id: string; name: string; category: string; body: string;
  notes: string | null; page_refs: Array<{ source_name: string; page_start: number; page_end: number }>;
  stale: boolean; }
compileCollection(collectionId: string): Promise<CompileSummary>   // B1b (B2a makes it also compile rules)
cancelCompile(): Promise<void>                                     // B1b
getCodexStatus(collectionId: string): Promise<CodexStatus>         // B1b
compileEntity(kind: string, id: string): Promise<boolean>          // B1b
getRuleEntries(collectionId: string): Promise<RuleEntry[]>         // B2b
updateRuleNotes(id: string, notes: string | null): Promise<void>   // B2b
redoRuleEntry(id: string, objection: string): Promise<void>        // B2b
```

Tauri event: `codex-progress` (payload `CompileProgress`). New `AppState` task slot: `compile_task: tokio::sync::Mutex<Option<tokio::task::AbortHandle>>` (mirrors `extract_task`).

`GraphNode`/`GraphNodeRecord` (entity_service/types.rs) gain `codex_article: Option<String>`, `codex_stale: Option<bool>`, `codex_compiled_at: Option<String>` — SCHEMAFULL SELECT * already returns them; only the structs and their record→node conversion change. Frontend `GraphNode` type in commands.ts mirrors this.

---

# PR B1a — `feat/b1a-setting-compile`

Setting-compile backend: article generation with scoped provenance, staleness clearing, re-embedding with the article, status query, Tauri command with progress + cancel.

### Task 1: Module split + `codex_status` (TDD)

**Files:**
- Create: `crates/chronacle-extraction/src/codex_service/status.rs`
- Modify: `crates/chronacle-extraction/src/codex_service/mod.rs` (becomes the module root: keep `mark_entity_stale`, `record_lint` and their tests; add `mod status; mod compile; mod prompts;` declarations as those files land; re-export public items)

**Interfaces:**
- Produces: `codex_status(db, collection_id) -> Result<CodexStatus, String>` with the struct above (`#[derive(Debug, Clone, serde::Serialize)]`).

- [ ] **Step 1: Create the branch**

```bash
git checkout --no-track -b feat/b1a-setting-compile main
```

- [ ] **Step 2: Write the failing tests** (in `status.rs`'s `#[cfg(test)] mod tests`, using the crate's mem-DB + `chronacle_db::run_migrations` idiom from `codex_service/mod.rs`):

```rust
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
```

- [ ] **Step 3: Run to verify failure** — `cargo test -p chronacle-extraction status_counts` → FAIL (fn missing).

- [ ] **Step 4: Implement `status.rs`**

```rust
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
```

Register `mod status;` + `pub use status::{codex_status, CodexStatus};` in `codex_service/mod.rs`. If `SELECT ... FROM $ents WHERE ...` over a record array misbehaves, fall back to fetching the ids and issuing per-table counts — the test is the contract; report the deviation.

- [ ] **Step 5: Run** — `cargo test -p chronacle-extraction codex_service` → PASS; `cargo clippy -p chronacle-extraction --all-targets -- -D warnings` clean.

- [ ] **Step 6: Commit**

```bash
git add crates/chronacle-extraction/src/codex_service
git commit -m "feat(codex): collection status query for staleness badges"
```

### Task 2: GraphNode carries codex fields

**Files:**
- Modify: `crates/chronacle-extraction/src/entity_service/types.rs` (`GraphNodeRecord`, `GraphNode`, the `From<GraphNodeRecord> for GraphNode` impl)
- Test: extend an existing read test in `crates/chronacle-extraction/src/entity_service/crud/crud_tests_read.rs`

**Interfaces:**
- Produces: `GraphNode { …, codex_article: Option<String>, codex_stale: Option<bool>, codex_compiled_at: Option<String> }` — consumed by Task 3 (compile skip logic), Task 6 (frontend types), B1b UI.

- [ ] **Step 1: Failing test** (append to `crud_tests_read.rs`, reusing its `setup_db`/`create` idioms):

```rust
#[tokio::test]
async fn get_by_id_exposes_codex_fields() {
    let db = setup_db().await;
    let node = create(
        &db,
        Some("camp1"),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Mira".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    db.query("UPDATE type::thing('npc', $id) SET codex_article = 'An article.', codex_stale = true")
        .bind(("id", node.id.clone()))
        .await
        .unwrap();
    let got = get_by_id(&db, &node.id, EntityKind::Npc).await.unwrap();
    assert_eq!(got.codex_article.as_deref(), Some("An article."));
    assert_eq!(got.codex_stale, Some(true));
}
```

- [ ] **Step 2: Verify failure** (fields don't exist on GraphNode) → **Step 3: Implement**: add the three fields to `GraphNodeRecord` (`codex_article: Option<String>`, `codex_stale: Option<bool>`, `codex_compiled_at: Option<surrealdb::sql::Datetime>`) and to `GraphNode` (`codex_compiled_at: Option<String>`, converted via `.map(|d| d.to_string())` in the From impl, matching how other timestamps convert — check the existing impl and mirror it). Fix any struct-literal construction sites (`GraphNode { ... }` in enrich.rs uses `..node.clone()`, so it compiles unchanged; compiler will name any others).

- [ ] **Step 4: Run** `cargo test -p chronacle-extraction && cargo clippy --workspace --all-targets --all-features -- -D warnings` → PASS/clean.

- [ ] **Step 5: Commit** — `feat(entity): expose codex fields on GraphNode`

### Task 3: Article compile pipeline (TDD)

**Files:**
- Create: `crates/chronacle-extraction/src/codex_service/prompts.rs`
- Create: `crates/chronacle-extraction/src/codex_service/compile.rs`
- Create: `crates/chronacle-extraction/src/codex_service/compile_tests.rs` (`#[cfg(test)] mod compile_tests;` registered in mod.rs)
- Modify: `crates/chronacle-extraction/src/codex_service/mod.rs` (module regs, re-exports, `CodexError`, phases/progress/result types, caps)

**Interfaces:**
- Consumes: `extraction_service`'s `MockLlm`/`MockEmbeddingProvider`/`MockVectorStore` (test_support — same crate), `search_passages`-style scoped retrieval via `Arc<dyn VectorStore>`, `entity_service::get_entity_relations` (1-hop neighbors), `codex_status` (Task 1), GraphNode codex fields (Task 2).
- Produces: `compile_collection`, `compile_entity`, `CodexError`, `CodexPhase`, `CompileProgress`, `CompileResult`, `MAX_COMPILE_PER_RUN` — exactly the shared-interface signatures above. Also `pub(crate) fn embed_entity_with_article` (embeds `name + summary + notes + article`, stores vector + model id; mirrors `entity_service::embed_node`'s zero-vector no-op guard).

- [ ] **Step 1: Add shared types to `mod.rs`**

```rust
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
pub enum CodexPhase { Resolving, Compiling, Embedding, Done, Empty }

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
    /// Entities still needing compile after the per-run cap.
    pub remaining_stale: usize,
}

/// Per-run cap on compiled entities (cost control; mirrors MAX_ENRICH).
pub const MAX_COMPILE_PER_RUN: usize = 50;
```

- [ ] **Step 2: Write the failing tests** (`compile_tests.rs`; the mock LLM returns the article body as plain text — the compile prompt asks for markdown prose, not JSON):

```rust
use std::sync::Arc;

use crate::codex_service::{compile_collection, compile_entity, CodexPhase, CompileProgress};
use crate::entity_service::{self, EntityInput, EntityKind};
use crate::extraction_service::test_support::{
    setup_db_with_collection, MockEmbeddingProvider, MockLlm, MockVectorStore,
};
use chronacle_core::embedding::EmbeddingProvider;
use chronacle_core::llm::LlmProvider;
use chronacle_core::vector_store::{SearchResult, VectorStore};

fn passage_hit(text: &str) -> SearchResult {
    SearchResult {
        chunk_id: "chunk:p1".into(),
        text: text.into(),
        source_name: "Core Rulebook".into(),
        page_start: 12,
        page_end: 13,
        // fill any remaining SearchResult fields with sensible defaults —
        // check the struct in chronacle-core::vector_store and mirror
        // MockVectorStore usage in extraction_service tests.
        ..Default::default()
    }
}

#[tokio::test]
async fn compile_writes_article_provenance_and_clears_stale() {
    let (db, col_id) = setup_db_with_collection().await;
    let node = entity_service::create(
        &db, None, Some(&col_id), EntityKind::Npc,
        EntityInput { name: "Mira".into(), summary: Some("An innkeeper.".into()), ..Default::default() },
    ).await.unwrap();
    db.query("UPDATE type::thing('npc', $id) SET codex_stale = true")
        .bind(("id", node.id.clone())).await.unwrap();

    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: "Mira runs the Gilded Flagon. [Source: \"Core Rulebook\", p.12]".into(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore {
        results: vec![passage_hit("Mira, innkeeper of the Gilded Flagon…")],
    });

    let res = compile_collection(&db, &llm, &embed, &vs, &col_id, |_| {}).await.unwrap();
    assert_eq!(res.articles_compiled, 1);
    assert_eq!(res.remaining_stale, 0);

    let got = entity_service::get_by_id(&db, &node.id, EntityKind::Npc).await.unwrap();
    assert!(got.codex_article.as_deref().unwrap_or("").contains("Gilded Flagon"));
    assert_eq!(got.codex_stale, Some(false));

    #[derive(serde::Deserialize)]
    struct C { count: i64 }
    let mut resp = db.query(
        "SELECT count() FROM npc WHERE codex_sources[0].source_name = 'Core Rulebook' \
           AND codex_sources[0].page_start = 12 GROUP ALL").await.unwrap();
    let rows: Vec<C> = resp.take(0).unwrap();
    assert_eq!(rows.first().map(|c| c.count).unwrap_or(0), 1, "chunk provenance must persist");
}

#[tokio::test]
async fn compile_skips_fresh_entities_and_makes_no_llm_calls() {
    let (db, col_id) = setup_db_with_collection().await;
    let node = entity_service::create(
        &db, None, Some(&col_id), EntityKind::Npc,
        EntityInput { name: "Mira".into(), ..Default::default() },
    ).await.unwrap();
    db.query("UPDATE type::thing('npc', $id) SET codex_stale = false, codex_article = 'done'")
        .bind(("id", node.id.clone())).await.unwrap();

    // A MockLlm that panics if called proves "nothing stale → no LLM cost".
    struct PanickingLlm;
    // implement LlmProvider for PanickingLlm with chat_stream that panics —
    // mirror MockLlm's impl in test_support and replace the body.
    let llm: Arc<dyn LlmProvider> = Arc::new(PanickingLlm);
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore { results: vec![] });

    let res = compile_collection(&db, &llm, &embed, &vs, &col_id, |_| {}).await.unwrap();
    assert_eq!(res.articles_compiled, 0);
}

#[tokio::test]
async fn compile_unset_stale_legacy_entity_is_included() {
    let (db, col_id) = setup_db_with_collection().await;
    let node = entity_service::create(
        &db, None, Some(&col_id), EntityKind::Npc,
        EntityInput { name: "Old One".into(), ..Default::default() },
    ).await.unwrap();
    db.query("UPDATE type::thing('npc', $id) UNSET codex_stale")
        .bind(("id", node.id.clone())).await.unwrap();

    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm { response: "Ancient.".into() });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore {
        results: vec![passage_hit("The Old One…")],
    });
    let res = compile_collection(&db, &llm, &embed, &vs, &col_id, |_| {}).await.unwrap();
    assert_eq!(res.articles_compiled, 1, "unset codex_stale must count as stale");
}

#[tokio::test]
async fn compile_emits_done_phase_with_counts() {
    let (db, col_id) = setup_db_with_collection().await;
    entity_service::create(
        &db, None, Some(&col_id), EntityKind::Npc,
        EntityInput { name: "Mira".into(), ..Default::default() },
    ).await.unwrap();
    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm { response: "Article.".into() });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore { results: vec![passage_hit("Mira…")] });
    let events = std::sync::Mutex::new(Vec::<CompileProgress>::new());
    compile_collection(&db, &llm, &embed, &vs, &col_id, |p| {
        events.lock().unwrap().push(p);
    }).await.unwrap();
    let events = events.into_inner().unwrap();
    assert_eq!(events.last().unwrap().phase, CodexPhase::Done);
    assert_eq!(events.last().unwrap().compiled, 1);
}

#[tokio::test]
async fn compile_entity_without_passages_returns_false_and_leaves_article() {
    let (db, col_id) = setup_db_with_collection().await;
    let node = entity_service::create(
        &db, None, Some(&col_id), EntityKind::Npc,
        EntityInput { name: "Ghost".into(), ..Default::default() },
    ).await.unwrap();
    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm { response: "unused".into() });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore { results: vec![] });
    let compiled = compile_entity(&db, &llm, &embed, &vs, "npc", &node.id).await.unwrap();
    assert!(!compiled, "no context → no article, no hallucinated compile");
}
```

If `SearchResult` lacks `Default`, construct it field-by-field (check the struct). If `MockVectorStore` isn't already `pub` enough, adjust visibility in test_support (same crate).

- [ ] **Step 3: Verify failures** — `cargo test -p chronacle-extraction compile_tests` → all FAIL (functions missing).

- [ ] **Step 4: Implement `prompts.rs`**

```rust
//! Prompt construction for codex article compilation.

/// Build the article-compilation prompt for one entity.
///
/// The LLM must ground every statement in the supplied passages and cite with
/// inline `[Source: "<name>", p.N]` markers; in-world entity names from the
/// neighbor list become `[[wikilinks]]`.
pub(super) fn build_article_prompt(
    name: &str,
    kind: &str,
    summary: Option<&str>,
    notes: Option<&str>,
    neighbors: &[(String, String)], // (name, rel_type)
    passages: &str,                 // pre-labeled: each passage prefixed with [Source: "...", p.X-Y]
) -> String {
    let neighbor_block = if neighbors.is_empty() {
        String::from("(none)")
    } else {
        neighbors
            .iter()
            .map(|(n, r)| format!("- [[{n}]] ({r})"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        r#"You are compiling the reference article for a TTRPG campaign codex.

Write the definitive article about the {kind} "{name}".

Rules:
- Use ONLY facts present in the source passages, the summary, or the notes below. NEVER invent facts.
- Cite every claim taken from a passage with its inline marker, exactly as given: [Source: "<name>", p.N]
- When you mention one of the related entities listed below, write its name as a [[wikilink]].
- Write flowing prose (2-6 paragraphs). No headings, no bullet lists, no preamble — start directly with the article text.

Known summary: {summary}
GM notes: {notes}
Related entities:
{neighbor_block}

Source passages:
{passages}"#,
        summary = summary.unwrap_or("(none)"),
        notes = notes.unwrap_or("(none)"),
    )
}
```

- [ ] **Step 5: Implement `compile.rs`**

Structure (write it out in full; key logic):

```rust
//! Setting-compile pipeline: stale entities → grounded codex articles.

use std::sync::Arc;
use surrealdb::Connection;

use super::{CodexError, CodexPhase, CompileProgress, CompileResult, MAX_COMPILE_PER_RUN};
use crate::entity_service::{self, GraphNode};
use chronacle_core::embedding::EmbeddingProvider;
use chronacle_core::llm::{ChatMessage, LlmProvider};
use chronacle_core::vector_store::VectorStore;

/// Resolve the collections an article compiled in `collection_id` may cite:
/// the owner campaign's full subscription set for a campaign-bound
/// collection; just the collection itself for a regular one (ADR-009).
async fn provenance_scope<C: Connection>(db, collection_id: &str) -> Result<Vec<String>, CodexError> {
    // SELECT VALUE owner_campaign FROM collection WHERE id = ...
    // if Some(cam): SELECT VALUE record::id(out) FROM subscribes_to WHERE in = $cam
    // else vec![collection_id]
    // NOTE: return BARE collection ids (the VectorStore::search contract takes
    // the same id shape resolve_collection_ids produces — check one call site
    // in agent_service and match it).
}

/// Entities in the collection needing compile (stale, unset, or article-less),
/// capped at MAX_COMPILE_PER_RUN; returns (targets, remaining_count).
async fn compile_targets<C: Connection>(db, collection_id) -> Result<(Vec<GraphNode>, usize), CodexError> {
    // LET $ents = (SELECT VALUE out FROM in_collection WHERE in = ...);
    // SELECT *, <SELECT_SCOPE_ALIASES equivalent> FROM $ents
    //   WHERE codex_stale != false OR codex_article = NONE;
    // Deserialize via entity_service's GraphNodeRecord path — add a
    // pub(crate) helper in entity_service if none exists (get_by_id per id is
    // acceptable: fetch ids first, then get_by_id each; simpler and reuses
    // tested code — prefer that).
}

/// Compile one entity: gather scoped passages + neighbors, prompt, persist,
/// re-embed. Returns false when no passage context exists (skip, not error).
async fn compile_one<C: Connection>(
    db, llm, embed, vector_store, node: &GraphNode, scope: &[String],
) -> Result<bool, CodexError> {
    // 1. query vec = embed(node.name + summary)  (embed_documents, first vec)
    // 2. hits = vector_store.search(&vec, scope, 8)
    //    if hits.is_empty() → Ok(false)
    // 3. passages block: for each hit →
    //    format!("[Source: \"{}\", p.{}-{}]\n{}", hit.source_name, hit.page_start, hit.page_end, hit.text)
    //    joined by "\n\n---\n\n"
    // 4. neighbors = entity_service::get_entity_relations(db, &node.kind, &node.id)
    //    mapped to (other_name, rel_type), capped at 12 — check the exact
    //    return type of get_entity_relations and adapt.
    // 5. article = llm_complete-style drain (copy the 15-line helper from
    //    extraction_service::llm_complete — it is pub(super) there; either
    //    make it pub(crate) in extraction_service/mod.rs or duplicate the
    //    small helper here with a comment; PREFER making it pub(crate)).
    // 6. persist: UPDATE type::thing($table,$id) SET codex_article = $article,
    //      codex_compiled_at = time::now(), codex_stale = false,
    //      codex_sources = <array of objects>;
    //    Build codex_sources as an inline SurrealQL array literal via
    //    sql-escaped fields? NO — use a plain #[derive(Serialize)] struct:
    //      struct ChunkSource { kind: &'static str, chunk: String,
    //                           source_name: String, page_start: i64, page_end: i64 }
    //    and bind Vec<ChunkSource> — plain structs serialize correctly
    //    (unlike serde_json::Value). Verify with the provenance test.
    // 7. embed_entity_with_article(db, embed, node, &article)
}

/// See module docs. `on_progress` receives Resolving → Compiling (per entity)
/// → Done|Empty.
pub async fn compile_collection<C: Connection>(
    db: &surrealdb::Surreal<C>,
    llm: &Arc<dyn LlmProvider>,
    embed: &Arc<dyn EmbeddingProvider>,
    vector_store: &Arc<dyn VectorStore>,
    collection_id: &str,
    on_progress: impl Fn(CompileProgress),
) -> Result<CompileResult, CodexError> {
    // resolve scope; fetch targets; loop compile_one with per-entity progress
    // events; count compiled; Done/Empty final event; CompileResult.
}

/// Compile a single entity by table + id (per-entity Recompile in the UI).
pub async fn compile_entity<C: Connection>(…) -> Result<bool, CodexError> {
    // get_by_id → resolve its collection's scope (via node.collection_id;
    // entities scoped only to a campaign use the campaign's subscription set)
    // → compile_one.
}

/// Embed name + summary + notes + article; zero-vector no-op like embed_node.
pub(crate) async fn embed_entity_with_article<C: Connection>(…) -> Result<(), CodexError> { … }
```

The implementer fills every `// …` with working code following the named patterns (each pattern's source is in this crate: `enrich.rs`, `edge.rs`, `update.rs`). No TODOs may remain.

- [ ] **Step 6: Run** — `cargo test -p chronacle-extraction` all green; workspace clippy clean.

- [ ] **Step 7: Commit** — `feat(codex): setting-compile pipeline with scoped provenance`

### Task 4: Tauri command + cancel + status command

**Files:**
- Create: `apps/desktop/src-tauri/src/commands/codex_commands.rs`
- Modify: `apps/desktop/src-tauri/src/commands/mod.rs` (module + re-exports), `apps/desktop/src-tauri/src/lib.rs` (AppState `compile_task` slot + `invoke_handler` registration — mirror `extract_task` exactly)
- Test: unit tests inside `codex_commands.rs` limited to pure helpers (the command bodies mirror `extraction_commands.rs`, which has the same test shape)

**Interfaces:**
- Produces: commands `compile_collection(collection_id) -> CompileSummary { articles_compiled, remaining_stale }`, `compile_entity(kind, id) -> bool`, `cancel_compile()`, `get_codex_status(collection_id) -> CodexStatus`; event `codex-progress`.

- [ ] **Step 1: Implement** — copy the `extract_all_from_campaign` structure: clone providers out of the RwLocks, `tokio::spawn` the compile, store the abort handle in `state.compile_task`, emit `codex-progress` via `app.emit`, map cancellation to `Err("cancelled")`. `get_codex_status` is a thin passthrough to `codex_service::codex_status`. `cancel_compile` calls `crate::commands::cancel_chat_task(&state.compile_task)`.
- [ ] **Step 2: Run** `cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace` → clean/green.
- [ ] **Step 3: Commit** — `feat(commands): codex compile + status + cancel`

### Task 5: Verify, push, PR (B1a)

- [ ] Full gate (Global Constraints command), then:

```bash
git push -u origin feat/b1a-setting-compile
gh pr create --title "feat: codex setting-compile pipeline (B1a)" --body "$(cat <<'EOF'
## What
codex_service compile pipeline: scoped provenance retrieval, grounded article prompt, staleness-aware target selection (unset counts as stale), provenance persistence, article-inclusive re-embedding; codex_status query; Tauri commands compile_collection/compile_entity/get_codex_status/cancel_compile with codex-progress events.

## Why
Codex spec PR-B1a. Spec: docs/superpowers/specs/2026-07-03-codex-compiled-world-model-design.md.

## Tested
cargo test --workspace (compile pipeline matrix incl. no-LLM-when-fresh, unset-stale inclusion, provenance round-trip, progress events; backend-only BDD per features/README.md convention).

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

# PR B1b — `feat/b1b-compile-ui`

Compile button + staleness badge in the collection row; Codex Article section + Recompile in the entity panel; acceptance feature. Branch from B1a's branch if unmerged (`git checkout --no-track -b feat/b1b-compile-ui feat/b1a-setting-compile`; PR base = that branch), else from main.

### Task 6: commands.ts wrappers + GraphNode codex fields

**Files:**
- Modify: `apps/desktop/src/lib/commands.ts`

- [ ] Add the TS interfaces + wrappers from Shared Interfaces (`CodexPhase`, `CompileProgress`, `CodexStatus`, `CompileSummary { articles_compiled: number; remaining_stale: number }`, `compileCollection`, `compileEntity`, `cancelCompile`, `getCodexStatus`), and extend the existing `GraphNode` interface with `codex_article: string | null`, `codex_stale: boolean | null`, `codex_compiled_at: string | null` (locate the interface and match its style; `invoke` keys: `{ collectionId }`, `{ kind, id }`).
- [ ] `pnpm -C apps/desktop typecheck` → PASS. Commit together with Task 7 or 8 if typecheck requires consumers; otherwise commit alone: `feat(ui): codex command wrappers`.

### Task 7: Collection row — Compile button + staleness badge (TDD)

**Files:**
- Modify: `apps/desktop/src/views/CampaignView.svelte` (the `.coll` row / expanded `.books` panel, ~lines 294-366)
- Test: `apps/desktop/src/views/CampaignView.test.ts`

**Interfaces:**
- Consumes: `getCodexStatus`, `compileCollection`, `cancelCompile`, `listen('codex-progress')`.
- Produces: per-collection "Compile" button (`aria-label="Compile {name}"`), badge text `{stale_entities} stale` when > 0 (class `codex-badge`), inline progress line while running, using the same visual language as the `index_status` badges.

- [ ] **Step 1: Failing Vitest tests** (module mock gains `getCodexStatus: vi.fn()`, `compileCollection: vi.fn()`, `cancelCompile: vi.fn()`; `@tauri-apps/api/event` is already mocked in the test setup or mock it with `listen: vi.fn().mockResolvedValue(() => {})` — check how OracleView tests mock it and mirror):

```ts
it('shows a stale badge and compile button per collection', async () => {
  m.getCollections.mockResolvedValue([col('c-1', 'World Guide')]);
  m.getCampaignCollections.mockResolvedValue([col('c-1', 'World Guide')]);
  m.getCodexStatus.mockResolvedValue({
    stale_entities: 12, total_entities: 40, rules_stale: 0, rule_entries: 0,
  });
  renderView();
  await waitFor(() => expect(screen.getByText('12 stale')).toBeTruthy());
  expect(screen.getByLabelText('Compile World Guide')).toBeTruthy();
});

it('compile button invokes compileCollection and refreshes status', async () => {
  m.getCollections.mockResolvedValue([col('c-1', 'World Guide')]);
  m.getCampaignCollections.mockResolvedValue([col('c-1', 'World Guide')]);
  m.getCodexStatus.mockResolvedValue({
    stale_entities: 1, total_entities: 1, rules_stale: 0, rule_entries: 0,
  });
  m.compileCollection.mockResolvedValue({ articles_compiled: 1, remaining_stale: 0 });
  renderView();
  await fireEvent.click(await screen.findByLabelText('Compile World Guide'));
  await waitFor(() => expect(m.compileCollection).toHaveBeenCalledWith('c-1'));
});
```

(`renderView()` = the file's existing render-with-default-props helper; add one if the file repeats inline props.)

- [ ] **Step 2: RED** → **Step 3: implement**: load `getCodexStatus` for subscribed collections alongside sources; render badge + button in `.coll-head`; on click set a per-collection `compiling` state, call `compileCollection`, listen to `codex-progress` for the detail line (subscribe in `onMount`, unlisten in `onDestroy`), refresh status on completion; surface errors in the view's existing `error` state. Cancel affordance: while compiling, the button becomes "Cancel" → `cancelCompile()`.
- [ ] **Step 4: GREEN** + `pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint`.
- [ ] **Step 5: Commit** — `feat(ui): per-collection codex compile with staleness badge`

### Task 8: Entity panel — Codex Article section + Recompile (TDD)

**Files:**
- Modify: `apps/desktop/src/components/EntityManager.svelte` (form panel, ~lines 225-246)
- Test: `apps/desktop/src/components/EntityManager.test.ts`

**Interfaces:**
- Consumes: `GraphNode.codex_article`/`codex_stale` (Task 6), `compileEntity`.
- Produces: read-only article block (heading "Codex Article", `WikiText` inside `.codex-article` with `white-space: pre-wrap`), "Stale" chip when `codex_stale !== false`, "Recompile" button (`aria-label="Recompile article"`). No edit affordance for the article, ever.

- [ ] **Step 1: Failing tests** (mirror the file's existing mocking style; the module mock gains `compileEntity: vi.fn()`):

```ts
it('renders the codex article read-only with a stale chip', async () => {
  // arrange the manager's entity fetch mock so the selected node carries
  // codex_article: 'Mira runs the [[Gilded Flagon]].', codex_stale: true
  // (follow the file's existing node-fixture helper)
  // assert: 'Codex Article' heading, article text visible, 'Stale' chip,
  // and NO textarea/input contains the article text.
});

it('recompile button calls compileEntity with kind and id', async () => {
  // click aria-label "Recompile article" → expect m.compileEntity
  // .toHaveBeenCalledWith('npc', '<id>')
});
```

Flesh these out against the file's real helpers (do not invent a parallel harness); the two assertions above are the contract.

- [ ] **Step 2-4: RED → implement → GREEN** (+ typecheck/lint). Section renders only when `codex_article` is non-null OR entity is compilable (always show Recompile; show "No article compiled yet" placeholder text when null).
- [ ] **Step 5: Commit** — `feat(ui): codex article panel with per-entity recompile`

### Task 9: Acceptance feature + verify, push, PR (B1b)

**Files:**
- Create: `apps/desktop/tests/e2e/features/codex-compile.feature`
- Create: `apps/desktop/tests/e2e/backend/steps/codex.steps.ts`
- Modify: `apps/desktop/tests/e2e/backend/ipc-mock.ts` (add cases: `get_codex_status` → `{ stale_entities: 12, total_entities: 40, rules_stale: 0, rule_entries: 0 }`, `compile_collection` → `{ articles_compiled: 12, remaining_stale: 0 }`, and make `get_collections`/`get_campaign_collections` return one collection `{ id: 'col1', name: 'World Guide', description: null }`)

- [ ] **Step 1: Feature** (verbatim from the spec's B1 scenarios, tightened):

```gherkin
Feature: Codex compilation
  The GM compiles a collection's codex explicitly; staleness badges show
  what is pending (ADR-009: manual compile, never automatic).

  Background:
    Given the app is running with a seeded campaign "Test Campaign"
    When the GM opens the app
    And the GM opens the campaign manager

  Scenario: A collection with stale entities shows its badge and compiles
    Then the collection "World Guide" shows the codex badge "12 stale"
    When the GM clicks compile on the collection "World Guide"
    Then the compile command is sent for the collection "World Guide"
```

- [ ] **Step 2-3: bddgen RED → steps** (reuse `steps/fixtures.ts`; assert the badge text, click `[aria-label="Compile World Guide"]`, then assert `window.__ipcCalls` contains `compile_collection` with `args.collectionId === 'col1'`).
- [ ] **Step 4: Full gate + push + PR**

```bash
gh pr create --title "feat: codex compile UI — badges, button, article panel (B1b)" --body "$(cat <<'EOF'
## What
Per-collection Compile button + staleness badge (codex-progress aware, cancellable); read-only Codex Article section with per-entity Recompile in the entity panel; executable acceptance feature.

## Why
Codex spec PR-B1b. Spec: docs/superpowers/specs/2026-07-03-codex-compiled-world-model-design.md.

## Tested
Vitest (badge/button/article-panel suites), pnpm run e2e:backend (codex-compile.feature), full workspace gate.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

(Add `--base feat/b1a-setting-compile` if B1a is unmerged.)

---

# PR B2a — `feat/b2a-rules-compile`

Rules pipeline: classify rules/supplement chunks into `rule_entry` rows, dedup-merge, redo-with-objections, list/notes commands; `compile_collection` command extended to run rules after articles.

### Task 10: Rules compile pipeline (TDD)

**Files:**
- Create: `crates/chronacle-extraction/src/codex_service/rules.rs`
- Create: `crates/chronacle-extraction/src/codex_service/rules_tests.rs`
- Modify: `crates/chronacle-extraction/src/codex_service/prompts.rs` (rules prompt), `mod.rs` (regs, `RulesCompileResult`, `RuleEntry` DTO, `MAX_RULE_BATCHES_PER_RUN`)

**Interfaces:**
- Produces: `compile_rules`, `redo_rule_entry`, `list_rule_entries`, `update_rule_notes`, `RuleEntry { id, name, category, body, notes, page_refs, stale }` (Serialize + Deserialize), `RulesCompileResult`.

- [ ] **Step 1: Rules prompt** in `prompts.rs` — the category definitions verbatim from the spec, including the disambiguation few-shot:

```rust
pub(super) const RULE_CATEGORY_DEFS: &str = "Rule categories (choose the single best fit):
- mechanic: a discrete rule or subsystem (initiative, opposed checks, downtime).
- ability: a named capability an actor can use (spell, feat, technique, power, maneuver).
- state: a condition or status affecting an actor (poisoned, exhausted, hunted).
- procedure: a step-by-step sequence (character creation, long rest, chase scene).
- resource: a countable in-play thing that is spent or restored during play (hit points, mana, stress, ammo).
- statistic: a numerical value used or modified in or by another rule (armor class, movement speed, carrying capacity). NOTE: hit points are a resource (spent/restored); armor class is a statistic (referenced/modified) — do not confuse the two.
- entry: freeform fallback when nothing above fits.";
```

`build_rules_prompt(labeled_chunks: &str) -> String`: instructs extraction of DISCRETE rules from the labeled passages (each passage is prefixed `[Source: "<name>", p.X-Y]`), skipping pure lore, returning ONLY JSON:

```json
{ "entries": [ { "name": "…", "category": "mechanic|ability|state|procedure|resource|statistic|entry",
                 "body": "self-contained rule text",
                 "page_refs": [ { "source_name": "…", "page_start": 1, "page_end": 2 } ] } ] }
```

`build_rules_redo_prompt(entry_name, current_body, objections: &[String], labeled_chunks)` — regenerate ONE entry honoring every listed GM objection.

- [ ] **Step 2: Failing tests** (`rules_tests.rs`; seed chunks directly — `setup_db_with_collection` seeds a collection; add chunk rows with `source_type`, `page_start/end`, and a `source` record so page labeling works — copy the chunk-seeding SurrealQL from `apps/desktop/src-tauri/tests/` fixtures if present, else write CREATE statements matching `001_base_schema.surql`'s chunk fields):

```rust
#[tokio::test]
async fn rules_compile_creates_entries_with_categories_and_page_refs() { /* rules-type
    chunk seeded; MockLlm returns one 'mechanic' entry with page_refs; assert
    rule_entry row exists with category, body, page_refs[0].page_start, stale=false,
    embedding present (MockEmbeddingProvider) */ }

#[tokio::test]
async fn rules_compile_skips_lore_only_sources() { /* only a 'lore' source_type chunk
    → PanickingLlm proves no LLM call; result 0 created */ }

#[tokio::test]
async fn rules_recompile_merges_by_name_preserving_notes() { /* pre-create entry with
    notes='table ruling'; MockLlm re-emits same name with new body; compile_rules;
    assert body updated, notes preserved, no duplicate row */ }

#[tokio::test]
async fn invalid_llm_category_falls_back_to_entry() { /* MockLlm emits category 'vibe';
    assert stored category == 'entry' (schema ASSERT would reject 'vibe') */ }

#[tokio::test]
async fn redo_rule_entry_stores_objection_and_regenerates() { /* create entry; call
    redo_rule_entry(.., "the range is wrong"); assert body regenerated (mock), notes
    preserved, sources contains { kind: 'objection', text: 'the range is wrong' } */ }
```

Write each body in full against the harness (the compile_tests.rs patterns from Task 3 transfer directly).

- [ ] **Step 3: RED → Step 4: implement `rules.rs`**: chunk query (`SELECT text, page_start, page_end, source_type, source.display_name AS source_name FROM chunk WHERE collection = … AND source_type IN ['rules','supplement']`), batch chunks under `BATCH_CHAR_BUDGET`-equivalent keeping labels, tolerant JSON parse (mirror `extraction_service::parse`'s fence-stripping — reuse via `pub(crate)` where possible), category whitelist with `entry` fallback, dedup-merge by `(collection, name)` (UPDATE body/category/compiled_at/stale=false + `array::union` page_refs, preserve notes; else CREATE), embed `name + category + body` via a `#[derive(Serialize)]` page-ref struct binding (never `serde_json::Value`). `redo_rule_entry`: fetch entry + its stored objections from `sources`, lexical chunk search by entry name within the collection, redo prompt, update body/category/page_refs + append `{ kind: 'objection', text, at: time::now() }` to sources. `list_rule_entries` / `update_rule_notes` are simple queries.
- [ ] **Step 5: GREEN + workspace clippy. Commit** — `feat(codex): rules compile with dedup-merge and objections`

### Task 11: Command layer: compile runs rules too; rules CRUD commands

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands/codex_commands.rs`

- [ ] Extend the `compile_collection` command's spawned task: after the article compile, run `codex_service::compile_rules` (same progress callback, phases reported through `codex-progress` with detail strings like "Compiling rules…"); `CompileSummary` gains `entries_created: usize, entries_updated: usize`. Add commands `get_rule_entries(collection_id) -> Vec<RuleEntry>`, `update_rule_notes(id, notes)`, `redo_rule_entry(id, objection)` (redo runs inline, not spawned — single-entry latency is acceptable). Register in `invoke_handler`.
- [ ] Workspace gate green. Commit — `feat(commands): rules compile + rule-entry management`

### Task 12: Verify, push, PR (B2a)

```bash
gh pr create --title "feat: codex rules compile — classification, merge, objections (B2a)" --body "$(cat <<'EOF'
## What
Rules pipeline over rules/supplement chunks: 7-category classification (resource-vs-statistic few-shot), dedup-merge by (collection,name) preserving GM notes, redo-with-objections with durable objection provenance; compile_collection now compiles rules after articles; rule-entry list/notes/redo commands.

## Why
Codex spec PR-B2a. Spec: docs/superpowers/specs/2026-07-03-codex-compiled-world-model-design.md.

## Tested
cargo test --workspace (rules matrix: creation, lore-skip, merge-preserves-notes, category fallback, objection round-trip — backend-only BDD per features/README.md).

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

(`--base feat/b1a-setting-compile` if stacking applies.)

---

# PR B2b — `feat/b2b-rules-ui`

Rules tab inside the expanded collection panel: category-grouped list, search, entry detail with GM notes + redo dialog.

### Task 13: commands.ts rules wrappers

- [ ] Add `RuleEntry`, `getRuleEntries`, `updateRuleNotes`, `redoRuleEntry` (Shared Interfaces) to `apps/desktop/src/lib/commands.ts`; typecheck; commit with Task 14 if needed.

### Task 14: Rules tab in the collection panel (TDD)

**Files:**
- Create: `apps/desktop/src/components/RulesPanel.svelte`
- Modify: `apps/desktop/src/views/CampaignView.svelte` (expanded `.coll` panel gains a `[Books | Rules]` tab strip; Books = existing sources list unchanged; Rules = `<RulesPanel collectionId={c.id} />`)
- Test: `apps/desktop/src/components/RulesPanel.test.ts`

**Interfaces:**
- Produces: `RulesPanel` props `{ collectionId: string }`. Renders: search input (`aria-label="Search rules"`), entries grouped under category headings (order: mechanic, ability, state, procedure, resource, statistic, entry; empty groups hidden), each entry expandable to show `body` (pre-wrap), page refs (`{source_name} p.{start}-{end}`), a GM-notes textarea (`aria-label="Table notes"`, saved on blur via `updateRuleNotes`), a stale chip, and "Redo with objections…" opening an inline dialog (textarea `aria-label="Objection"` + Submit → `redoRuleEntry(id, text)` → reload).

- [ ] **Step 1: Failing tests**:

```ts
it('groups entries by category and filters by search', async () => {
  m.getRuleEntries.mockResolvedValue([
    rule('r1', 'Initiative', 'mechanic'),
    rule('r2', 'Fireball', 'ability'),
  ]);
  render(RulesPanel, { props: { collectionId: 'c-1' } });
  await waitFor(() => expect(screen.getByText('Initiative')).toBeTruthy());
  expect(screen.getByRole('heading', { name: /mechanic/i })).toBeTruthy();
  await fireEvent.input(screen.getByLabelText('Search rules'), { target: { value: 'fire' } });
  await waitFor(() => expect(screen.queryByText('Initiative')).toBeNull());
  expect(screen.getByText('Fireball')).toBeTruthy();
});

it('saves table notes on blur and submits objections', async () => {
  m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
  render(RulesPanel, { props: { collectionId: 'c-1' } });
  await fireEvent.click(await screen.findByText('Initiative'));
  const notes = screen.getByLabelText('Table notes');
  await fireEvent.input(notes, { target: { value: 'we roll once per round' } });
  await fireEvent.blur(notes);
  await waitFor(() =>
    expect(m.updateRuleNotes).toHaveBeenCalledWith('r1', 'we roll once per round'),
  );
  await fireEvent.click(screen.getByText(/Redo with objections/));
  await fireEvent.input(screen.getByLabelText('Objection'), { target: { value: 'range is wrong' } });
  await fireEvent.click(screen.getByText('Submit'));
  await waitFor(() => expect(m.redoRuleEntry).toHaveBeenCalledWith('r1', 'range is wrong'));
});
```

(`rule(id, name, category)` = local fixture helper returning a full `RuleEntry`.)

- [ ] **Step 2-4: RED → implement → GREEN** (+ typecheck/lint). The component owns its data-loading (`getRuleEntries` on mount / after redo), grouping (`$derived`), and search state; body text renders via `WikiText`-style pre-wrap (plain `<p class="body">` with `white-space: pre-wrap` is fine — rule bodies have no wikilinks contract yet).
- [ ] **Step 5: Commit** — `feat(ui): rules tab with categories, notes, redo dialog`

### Task 15: Acceptance feature + verify, push, PR (B2b)

**Files:**
- Create: `apps/desktop/tests/e2e/features/rules-tab.feature`
- Modify: `apps/desktop/tests/e2e/backend/steps/codex.steps.ts` (new steps), `ipc-mock.ts` (`get_rule_entries` → one mechanic "Initiative" entry with page_refs; `update_rule_notes`/`redo_rule_entry` → null)

- [ ] **Feature:**

```gherkin
Feature: Compiled rules browsing
  Compiled rules are browsable per collection, grouped by category, with
  GM-owned table notes and redo-with-objections (ADR-009).

  Background:
    Given the app is running with a seeded campaign "Test Campaign"
    When the GM opens the app
    And the GM opens the campaign manager
    And the GM opens the rules tab of collection "World Guide"

  Scenario: Rule entries are grouped by category with page references
    Then the rules list shows "Initiative" under the "mechanic" category
    And the entry "Initiative" cites "Core Rulebook p.12-13"

  Scenario: The GM disputes a rule with an objection
    When the GM opens the rule entry "Initiative"
    And the GM submits the objection "the range is wrong"
    Then a redo command is sent for the entry "Initiative"
```

- [ ] Steps via `__ipcCalls` assertions as before; full gate; push; PR:

```bash
gh pr create --title "feat: rules tab — categories, notes, redo (B2b)" --body "$(cat <<'EOF'
## What
Rules tab inside each collection panel: category-grouped compiled rules with search, page refs, GM table notes, and redo-with-objections dialog; acceptance feature.

## Why
Codex spec PR-B2b. Spec: docs/superpowers/specs/2026-07-03-codex-compiled-world-model-design.md.

## Tested
Vitest RulesPanel suite; pnpm run e2e:backend (rules-tab.feature); full workspace gate.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Docs shipped inside these PRs

- B1a: architecture.md "RAG Pipeline" section gains a short "Codex compilation" paragraph (manual, scoped provenance, caps) — one commit inside PR B1a (`docs(architecture): codex compile pipeline notes`).
- B2b: `docs/user-guide.md` gains "The Codex" chapter covering compiling (cost + staleness badges), articles vs notes, the seven rule categories in GM terms (resource-vs-statistic contrast), table notes, redo-with-objections — one commit inside PR B2b (`docs(guide): the Codex chapter`), written for GMs, no jargon.

## Execution notes

- Order: B1a → (B1b, B2a in either order; B2a depends only on B1a) → B2b (needs B2a).
- Reconciliation contingency (rebase-merge repo): after any upstream PR merges, rebase in-flight branches onto the new main / new upstream branch tip (`git rebase --onto`), force-push with lease, re-record task-base SHAs before generating review packages (a stale base silently inflates diffs).
- The B1 compiler treats unset `codex_stale` as stale — this is the A2a carry-forward and is pinned by `compile_unset_stale_legacy_entity_is_included`.
- Known SurrealQL hazards: never KNN + `id IN (subquery)`; never bind `serde_json::Value`/`sql::Value` payloads (use plain Serialize structs or inline literals); `SELECT ... FROM $array_of_records` fallback strategy is per-id `get_by_id`.
