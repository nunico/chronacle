# `/extract` Chat Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the per-collection "Extract entities" button with chat commands — `/extract <name>` (seed-anchored targeted extraction), `/extract-all` (cancellable full sweep), and `/help` — backed by a live in-thread progress card so the user always knows what is happening.

**Architecture:** A pure parser in the frontend routes slash-commands before they reach `chatSend`. Two new Tauri commands run extraction as spawned, abortable tasks (mirroring the existing `chat_send`/`chat_cancel` pattern) and emit phased `extract-progress` events. The Rust extraction service gains a shared `persist_batch` helper, a seed-anchored prompt, and an `extract_seed_anchored` entry point that gathers passages per-collection via the `VectorStore` trait plus a lexical scan. An `ExtractionCard.svelte` renders phases, a cancel button, and terminal states.

**Tech Stack:** Rust, Tauri 2, SurrealDB (in-mem for tests), Svelte 5 (runes), Vitest + @testing-library/svelte.

**Spec:** `docs/superpowers/specs/2026-06-13-extract-chat-command-design.md`

---

## File Structure

**Backend (Rust)**
- `src-tauri/src/services/extraction_service.rs` — change `ExtractionProgress` to a phased shape; add `persist_batch` helper, `build_seed_prompt`, `extract_seed_anchored`; refactor `extract_from_collection` to use the helper and emit phased progress.
- `src-tauri/src/commands/extraction_commands.rs` — replace `extract_entities_from_collection` with `extract_entity_by_name`, `extract_all_from_campaign`, and `cancel_extraction`; spawn + register abort handle; emit phased events.
- `src-tauri/src/lib.rs` — add `extract_task` field to `AppState` (+ init); register the three commands; deregister the removed one.

**Frontend (Svelte/TS)**
- `src/lib/chat-commands.ts` (new) — pure `parseCommand` parser.
- `src/lib/chat-commands.test.ts` (new) — parser table tests.
- `src/lib/commands.ts` — replace extraction bindings + `ExtractionProgress` type.
- `src/components/ExtractionCard.svelte` (new) — phase checklist, spinner, cancel button, terminal states.
- `src/components/ExtractionCard.test.ts` (new) — card render tests.
- `src/views/OracleView.svelte` — route commands, listen to `extract-progress`, render the card, wire cancel.
- `src/views/OracleView.test.ts` — routing tests.
- `src/views/CampaignView.svelte` — remove the old extract button and its logic.

---

## Task 1: Phased progress + shared persist helper (backend service)

Refactor the service so progress is phase-based and the per-batch persistence logic is shared by both the existing sweep and the new seed-anchored path.

**Files:**
- Modify: `src-tauri/src/services/extraction_service.rs`

- [ ] **Step 1: Write a failing test for the new phased progress shape**

Add to the `tests` module in `src-tauri/src/services/extraction_service.rs`:

```rust
#[tokio::test]
async fn extract_from_collection_emits_done_phase_with_cumulative_counts() {
    let (db, col_id) = setup_db_with_collection().await;
    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: r#"{"entities":[{"name":"The Iron Fist","kind":"faction","summary":"x","notes":null,"relations":[{"name":"Commander Varn","kind":"npc","rel_type":"commands","summary":"y","notes":null}]}]}"#.to_string(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));

    let phases = std::sync::Mutex::new(Vec::<ExtractionProgress>::new());
    extract_from_collection(&db, &llm, &embed, &col_id, |p| {
        phases.lock().unwrap().push(p);
    })
    .await
    .unwrap();

    let phases = phases.into_inner().unwrap();
    let done = phases.last().expect("at least one progress event");
    assert_eq!(done.phase, ExtractionPhase::Done);
    assert_eq!(done.entities_found, 2);
    assert_eq!(done.relations_found, 1);
}
```

- [ ] **Step 2: Run the test to verify it fails to compile**

Run: `cargo test --manifest-path src-tauri/Cargo.toml extract_from_collection_emits_done_phase -- --nocapture`
Expected: FAIL — `ExtractionPhase` undefined, `ExtractionProgress` has no `phase`/`relations_found` fields.

- [ ] **Step 3: Replace the `ExtractionProgress` struct with the phased shape**

In `src-tauri/src/services/extraction_service.rs`, replace the existing `ExtractionProgress` definition (the `batch`/`total_batches`/`entities_found` struct) with:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionPhase {
    Resolving,
    Searching,
    Extracting,
    Relating,
    Embedding,
    Done,
    Empty,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtractionProgress {
    pub phase: ExtractionPhase,
    /// Human-readable, e.g. "Found 12 passages".
    pub detail: String,
    /// Running total across the whole extraction run.
    pub entities_found: usize,
    /// Running total across the whole extraction run.
    pub relations_found: usize,
}
```

- [ ] **Step 4: Extract the per-batch persistence into a shared helper**

Add this function above `extract_from_collection`. It contains the dedup/create/embed/relate logic currently inlined in the batch loop:

```rust
/// Persist one parsed LLM batch into `collection_id`, deduplicating by
/// name+kind within the collection. Returns (entities_created, relations_created)
/// and pushes any newly created nodes onto `all_nodes`.
async fn persist_batch<C: Connection>(
    db: &surrealdb::Surreal<C>,
    embed: &Arc<dyn EmbeddingProvider>,
    collection_id: &str,
    parsed: &LlmResponse,
    all_nodes: &mut Vec<GraphNode>,
) -> Result<(usize, usize), ExtractionError> {
    let mut entities_created = 0usize;
    let mut relations_created = 0usize;

    for ent in &parsed.entities {
        let kind = parse_kind(&ent.kind);
        let existing =
            entity_service::find_by_name_and_collection(db, collection_id, &ent.name, kind.clone())
                .await
                .map_err(|e| ExtractionError::Db(e.to_string()))?;

        let origin_node = if let Some(node) = existing {
            node
        } else {
            let node = entity_service::create(
                db,
                None,
                Some(collection_id),
                kind,
                EntityInput {
                    name: ent.name.clone(),
                    summary: ent.summary.clone(),
                    notes: ent.notes.clone(),
                    date_start: None,
                    date_end: None,
                    is_ongoing: None,
                    sequence_index: None,
                    era: None,
                    duration_label: None,
                    session_id: None,
                    player_name: None,
                    character_class: None,
                    character_level: None,
                    status: None,
                },
            )
            .await
            .map_err(|e| ExtractionError::Db(e.to_string()))?;
            let _ = embed_entity(db, embed, &node).await;
            entities_created += 1;
            all_nodes.push(node.clone());
            node
        };

        for rel in &ent.relations {
            let rel_kind = parse_kind(&rel.kind);
            let existing_rel = entity_service::find_by_name_and_collection(
                db,
                collection_id,
                &rel.name,
                rel_kind.clone(),
            )
            .await
            .map_err(|e| ExtractionError::Db(e.to_string()))?;

            let rel_node = if let Some(node) = existing_rel {
                node
            } else {
                let node = entity_service::create(
                    db,
                    None,
                    Some(collection_id),
                    rel_kind,
                    EntityInput {
                        name: rel.name.clone(),
                        summary: rel.summary.clone(),
                        notes: rel.notes.clone(),
                        date_start: None,
                        date_end: None,
                        is_ongoing: None,
                        sequence_index: None,
                        era: None,
                        duration_label: None,
                        session_id: None,
                        player_name: None,
                        character_class: None,
                        character_level: None,
                        status: None,
                    },
                )
                .await
                .map_err(|e| ExtractionError::Db(e.to_string()))?;
                let _ = embed_entity(db, embed, &node).await;
                entities_created += 1;
                all_nodes.push(node.clone());
                node
            };

            if rel_node.campaign_id.is_some() {
                eprintln!(
                    "extraction: skipping cross-link {} → {} (collection→campaign forbidden)",
                    origin_node.name, rel_node.name
                );
                continue;
            }

            let result = entity_service::relate(
                db,
                &origin_node.id,
                &origin_node.kind,
                &rel_node.id,
                &rel_node.kind,
                &rel.rel_type,
                None,
            )
            .await;
            if result.is_ok() {
                relations_created += 1;
            }
        }
    }

    Ok((entities_created, relations_created))
}
```

- [ ] **Step 5: Rewrite `extract_from_collection`'s batch loop to use the helper and emit phased progress**

Replace the body of the `for (batch_idx, chunk_text) in batches.iter().enumerate()` loop (and the trailing `on_progress` call inside it) with:

```rust
    for (batch_idx, chunk_text) in batches.iter().enumerate() {
        let user_prompt = build_extraction_prompt(chunk_text);
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: user_prompt,
        }];

        let raw = llm_complete(llm.as_ref(), system_prompt, &messages).await?;
        let parsed = parse_extraction_response(&raw);

        let (ec, rc) = persist_batch(db, embed, collection_id, &parsed, &mut all_nodes).await?;
        entities_created += ec;
        relations_created += rc;

        on_progress(ExtractionProgress {
            phase: ExtractionPhase::Extracting,
            detail: format!("Batch {}/{}", batch_idx + 1, total_batches),
            entities_found: entities_created,
            relations_found: relations_created,
        });
    }

    on_progress(ExtractionProgress {
        phase: ExtractionPhase::Done,
        detail: format!("Created {entities_created} entities, {relations_created} relations"),
        entities_found: entities_created,
        relations_found: relations_created,
    });
```

(Delete the now-duplicated dedup/create/relate code that previously lived inside this loop.)

- [ ] **Step 6: Run the full service test module**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib extraction_service`
Expected: PASS — the new test plus all four existing round-trip tests (`extract_creates_entities_with_collection_edge`, `extract_deduplicates_on_second_run`, `extract_level2_refs_stay_as_wikilinks_not_entities`, `extract_cross_link_collection_to_campaign_is_skipped`).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/services/extraction_service.rs
git commit -m "refactor: phased extraction progress + shared persist_batch helper"
```

---

## Task 2: Seed-anchored extraction prompt (backend service)

**Files:**
- Modify: `src-tauri/src/services/extraction_service.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
#[test]
fn build_seed_prompt_anchors_on_entity_name() {
    let prompt = build_seed_prompt("Commander Varn", "Varn leads the Iron Fist.");
    assert!(prompt.contains("Commander Varn"));
    assert!(prompt.contains("Varn leads the Iron Fist."));
    assert!(prompt.contains("entities"));
    assert!(prompt.contains("JSON"));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib build_seed_prompt_anchors`
Expected: FAIL — `build_seed_prompt` not found.

- [ ] **Step 3: Implement `build_seed_prompt`**

Add near `build_extraction_prompt`:

```rust
/// Build a seed-anchored extraction prompt: focus on `name` and the entities
/// directly related to it, rather than extracting everything in the text.
fn build_seed_prompt(name: &str, chunk_text: &str) -> String {
    format!(
        r#"You are an expert at extracting structured game entities from TTRPG source material.

Build a complete profile of the entity named "{name}" using ONLY the source text below.
- Output "{name}" as a single level-0 entity with its kind, a concise summary (1-2 sentences), and notes.
- Include entities DIRECTLY related to "{name}" in its "relations" array (allies, members, locations, leaders, etc.).
- For entities mentioned only in passing, write their names as [[wikilinks]] inside notes — do NOT extract them separately.
- If "{name}" is not described in the text, return an empty "entities" array.

Return ONLY valid JSON matching this exact schema (no markdown, no explanation):

{{
  "entities": [
    {{
      "name": "string",
      "kind": "npc|location|faction|creature|item|event|player_character|misc",
      "summary": "string",
      "notes": "optional string, may contain [[wikilinks]]",
      "relations": [
        {{ "name": "string", "kind": "string", "rel_type": "string", "summary": "string", "notes": "optional string" }}
      ]
    }}
  ]
}}

Source text:
{chunk_text}"#
    )
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib build_seed_prompt_anchors`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/extraction_service.rs
git commit -m "feat: seed-anchored extraction prompt"
```

---

## Task 3: `extract_seed_anchored` entry point (backend service)

Gathers passages per campaign-collection (semantic via `VectorStore`, lexical via SurrealQL), runs the seed prompt, and persists collection-scoped.

**Files:**
- Modify: `src-tauri/src/services/extraction_service.rs`

- [ ] **Step 1: Write the failing integration test**

Add to the `tests` module. This includes a test-local `MockVectorStore` (no shared mock exists yet):

```rust
use crate::providers::vector_store::{SearchResult, VectorStore, VectorStoreError, IndexedChunk};

struct MockVectorStore {
    results: Vec<SearchResult>,
}

#[async_trait::async_trait]
impl VectorStore for MockVectorStore {
    async fn upsert(&self, _s: &str, _c: &[IndexedChunk]) -> Result<(), VectorStoreError> {
        Ok(())
    }
    async fn search(
        &self,
        _q: &[f32],
        _cids: &[String],
        _limit: u64,
    ) -> Result<Vec<SearchResult>, VectorStoreError> {
        Ok(self.results.clone())
    }
    async fn delete_by_source(&self, _s: &str) -> Result<(), VectorStoreError> {
        Ok(())
    }
}

#[tokio::test]
async fn seed_anchored_builds_named_entity_and_relations_collection_scoped() {
    let (db, col_id) = setup_db_with_collection().await;

    // Link the collection to a campaign so resolve_collection_ids finds it.
    db.query(
        "CREATE campaign SET id='camp1', name='C', system='5e', created_at=time::now(), updated_at=time::now(); \
         RELATE campaign:camp1->subscribes_to->type::thing('collection', $cid) SET created_at=time::now()",
    )
    .bind(("cid", col_id.clone()))
    .await
    .unwrap();

    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: r#"{"entities":[{"name":"Commander Varn","kind":"npc","summary":"Leader.","notes":null,"relations":[{"name":"The Iron Fist","kind":"faction","rel_type":"commands","summary":"Militia.","notes":null}]}]}"#.to_string(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore { results: vec![] });

    let result = extract_seed_anchored(&db, &llm, &embed, &vs, "camp1", "Commander Varn", |_| {})
        .await
        .unwrap();

    // Lexical scan matches the fixture chunk text ("Commander Varn leads them.").
    assert_eq!(result.entities_created, 2);
    assert_eq!(result.relations_created, 1);

    let npcs = entity_service::get_by_collection(&db, &col_id, EntityKind::Npc).await.unwrap();
    assert!(npcs.iter().any(|n| n.name == "Commander Varn"));
}

#[tokio::test]
async fn seed_anchored_emits_empty_phase_when_no_passages() {
    let (db, col_id) = setup_db_with_collection().await;
    db.query(
        "CREATE campaign SET id='camp1', name='C', system='5e', created_at=time::now(), updated_at=time::now(); \
         RELATE campaign:camp1->subscribes_to->type::thing('collection', $cid) SET created_at=time::now()",
    )
    .bind(("cid", col_id.clone()))
    .await
    .unwrap();

    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm { response: "{}".to_string() });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore { results: vec![] });

    let phases = std::sync::Mutex::new(Vec::<ExtractionProgress>::new());
    let result = extract_seed_anchored(&db, &llm, &embed, &vs, "camp1", "Nonexistent Entity", |p| {
        phases.lock().unwrap().push(p);
    })
    .await
    .unwrap();

    assert_eq!(result.entities_created, 0);
    let phases = phases.into_inner().unwrap();
    assert_eq!(phases.last().unwrap().phase, ExtractionPhase::Empty);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib seed_anchored`
Expected: FAIL — `extract_seed_anchored` not found.

- [ ] **Step 3: Implement `extract_seed_anchored`**

Add to the `// ── Public API ──` section. Note the imports at the top of the file must include `use crate::providers::vector_store::VectorStore;`:

```rust
/// Per-batch character budget reused for seed passages.
const SEED_SEARCH_K: u64 = 12;

/// Seed-anchored extraction: build the entity named `name` plus its relation
/// neighborhood from chunks across all collections linked to `campaign_id`.
///
/// For each linked collection it gathers candidate passages by the union of
/// semantic search (`VectorStore`) and a lexical `CONTAINS` scan, then runs the
/// seed-anchored prompt and persists collection-scoped (same dedup path as the
/// full sweep). Passing a single collection id to `search` guarantees every
/// semantic hit belongs to that collection, so scoping is unambiguous.
pub async fn extract_seed_anchored<C: Connection>(
    db: &surrealdb::Surreal<C>,
    llm: &Arc<dyn LlmProvider>,
    embed: &Arc<dyn EmbeddingProvider>,
    vector_store: &Arc<dyn VectorStore>,
    campaign_id: &str,
    name: &str,
    on_progress: impl Fn(ExtractionProgress),
) -> Result<ExtractionResult, ExtractionError> {
    on_progress(ExtractionProgress {
        phase: ExtractionPhase::Resolving,
        detail: format!("Resolving \"{name}\""),
        entities_found: 0,
        relations_found: 0,
    });

    let collection_ids = crate::services::agent_service::resolve_collection_ids(db, campaign_id)
        .await
        .map_err(|e| ExtractionError::Db(e.to_string()))?;

    let query_vec = embed
        .embed_documents(vec![name.to_string()])
        .await
        .map_err(|e| ExtractionError::Embedding(e.to_string()))?
        .into_iter()
        .next()
        .unwrap_or_default();

    let needle = name.to_lowercase();
    let mut entities_created = 0usize;
    let mut relations_created = 0usize;
    let mut all_nodes: Vec<GraphNode> = Vec::new();
    let mut total_passages = 0usize;

    for cid in &collection_ids {
        // 1. Semantic hits (all belong to `cid` because we pass a single id).
        let semantic = vector_store
            .search(&query_vec, std::slice::from_ref(cid), SEED_SEARCH_K)
            .await
            .map_err(|e| ExtractionError::Db(e.to_string()))?;

        // 2. Lexical hits within this collection.
        #[derive(serde::Deserialize)]
        struct Row {
            id: surrealdb::sql::Thing,
            text: String,
        }
        let mut resp = db
            .query(
                "SELECT id, text FROM chunk \
                 WHERE collection = type::thing('collection', $cid) \
                 AND string::lowercase(text) CONTAINS $needle",
            )
            .bind(("cid", cid.clone()))
            .bind(("needle", needle.clone()))
            .await
            .map_err(|e| ExtractionError::Db(e.to_string()))?;
        let lexical: Vec<Row> = resp.take(0).map_err(|e| ExtractionError::Db(e.to_string()))?;

        // 3. Union by chunk id, preserving text.
        let mut seen = std::collections::HashSet::new();
        let mut passages: Vec<String> = Vec::new();
        for r in &semantic {
            if seen.insert(r.chunk_id.clone()) {
                passages.push(r.text.clone());
            }
        }
        for r in lexical {
            if seen.insert(r.id.id.to_raw()) {
                passages.push(r.text);
            }
        }
        if passages.is_empty() {
            continue;
        }
        total_passages += passages.len();

        on_progress(ExtractionProgress {
            phase: ExtractionPhase::Searching,
            detail: format!("Found {total_passages} passages"),
            entities_found: entities_created,
            relations_found: relations_created,
        });

        // 4. Batch passages by char budget and run the seed prompt per batch.
        let mut batches: Vec<String> = Vec::new();
        let mut current = String::new();
        for p in passages {
            if !current.is_empty() && current.len() + p.len() > BATCH_CHAR_BUDGET {
                batches.push(std::mem::take(&mut current));
            }
            current.push_str(&p);
            current.push('\n');
        }
        if !current.is_empty() {
            batches.push(current);
        }

        let system_prompt =
            "You are a structured data extraction assistant. Return ONLY valid JSON.";
        for chunk_text in &batches {
            let messages = vec![ChatMessage {
                role: "user".to_string(),
                content: build_seed_prompt(name, chunk_text),
            }];
            let raw = llm_complete(llm.as_ref(), system_prompt, &messages).await?;
            let parsed = parse_extraction_response(&raw);

            on_progress(ExtractionProgress {
                phase: ExtractionPhase::Extracting,
                detail: format!("Building \"{name}\""),
                entities_found: entities_created,
                relations_found: relations_created,
            });

            let (ec, rc) = persist_batch(db, embed, cid, &parsed, &mut all_nodes).await?;
            entities_created += ec;
            relations_created += rc;
        }
    }

    if total_passages == 0 {
        on_progress(ExtractionProgress {
            phase: ExtractionPhase::Empty,
            detail: format!("No passages found for \"{name}\""),
            entities_found: 0,
            relations_found: 0,
        });
    } else {
        on_progress(ExtractionProgress {
            phase: ExtractionPhase::Done,
            detail: format!("Created {entities_created} entities, {relations_created} relations"),
            entities_found: entities_created,
            relations_found: relations_created,
        });
    }

    Ok(ExtractionResult {
        entities_created,
        relations_created,
        entities: all_nodes,
    })
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib seed_anchored`
Expected: PASS — both tests.

- [ ] **Step 5: Run the whole service module + clippy**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib extraction_service && cargo clippy --manifest-path src-tauri/Cargo.toml --lib -- -D warnings`
Expected: PASS, no warnings.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/services/extraction_service.rs
git commit -m "feat: extract_seed_anchored gathers passages per collection"
```

---

## Task 4: `extract_task` abort slot in AppState (backend)

**Files:**
- Modify: `src-tauri/src/lib.rs:25` (struct field) and `src-tauri/src/lib.rs:154` (init)

- [ ] **Step 1: Add the field to `AppState`**

After the `chat_task` field (line 25), add:

```rust
    /// Abort handle for the in-flight extraction task, if any (see `cancel_extraction`).
    pub extract_task: tokio::sync::Mutex<Option<tokio::task::AbortHandle>>,
```

- [ ] **Step 2: Initialize it where `AppState` is constructed**

Next to `chat_task: tokio::sync::Mutex::new(None),` (line 154), add:

```rust
        extract_task: tokio::sync::Mutex::new(None),
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: builds (the old `extract_entities_from_collection` command still exists and compiles; it is replaced in Task 5).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: add extract_task abort slot to AppState"
```

---

## Task 5: Replace extraction Tauri commands (backend)

**Files:**
- Modify: `src-tauri/src/commands/extraction_commands.rs` (replace file body)
- Modify: `src-tauri/src/lib.rs` (command registration around line 210)

- [ ] **Step 1: Write the failing test for cancellation reuse**

Add a `#[cfg(test)]` module at the bottom of `src-tauri/src/commands/extraction_commands.rs`:

```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn cancel_extraction_aborts_registered_task_and_empties_slot() {
        let slot: tokio::sync::Mutex<Option<tokio::task::AbortHandle>> =
            tokio::sync::Mutex::new(None);
        let task = tokio::spawn(async { loop { tokio::task::yield_now().await; } });
        *slot.lock().await = Some(task.abort_handle());

        assert!(crate::commands::cancel_chat_task(&slot).await);
        let err = task.await.expect_err("task should have been aborted");
        assert!(err.is_cancelled());
        assert!(!crate::commands::cancel_chat_task(&slot).await);
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml cancel_extraction_aborts_registered`
Expected: FAIL to compile — `cancel_chat_task` is `pub(crate)` (already), so this should actually compile and PASS once the module is added. If it FAILS on visibility, that signals the import path. Confirm PASS; this anchors that we reuse the existing helper. (No new helper needed.)

- [ ] **Step 3: Replace the body of `extraction_commands.rs`**

Replace the whole file with:

```rust
use std::sync::Arc;

use serde::Serialize;
use tauri::Emitter;
use tauri::State;

use crate::services::extraction_service::ExtractionProgress;
use crate::AppState;

/// Summary returned to the frontend when extraction completes.
#[derive(Debug, Clone, Serialize)]
pub struct ExtractionSummary {
    pub entities_created: usize,
    pub relations_created: usize,
}

/// Emit a phased progress event to the frontend.
fn emit_progress(app: &tauri::AppHandle, p: &ExtractionProgress) {
    let _ = app.emit("extract-progress", p);
}

/// Seed-anchored extraction of a single named entity across the active
/// campaign's collections. Runs as an abortable spawned task.
#[tauri::command]
pub async fn extract_entity_by_name(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
    name: String,
) -> Result<ExtractionSummary, String> {
    let state_ref = state.inner().clone();
    let llm = state_ref.llm_provider.read().map_err(|e| format!("LLM lock: {e}"))?.clone();
    let embed = state_ref.embedding_provider.read().map_err(|e| format!("Embed lock: {e}"))?.clone();
    let vector_store = state_ref.vector_store.clone();
    let app = app_handle.clone();

    let task = tokio::spawn(async move {
        crate::services::extraction_service::extract_seed_anchored(
            &state_ref.db,
            &llm,
            &embed,
            &vector_store,
            &campaign_id,
            &name,
            move |p| emit_progress(&app, &p),
        )
        .await
    });

    *state.extract_task.lock().await = Some(task.abort_handle());

    match task.await {
        Ok(Ok(result)) => Ok(ExtractionSummary {
            entities_created: result.entities_created,
            relations_created: result.relations_created,
        }),
        Ok(Err(e)) => Err(format!("Extraction failed: {e}")),
        Err(join_err) if join_err.is_cancelled() => Err("cancelled".to_string()),
        Err(join_err) => Err(format!("Extraction task error: {join_err}")),
    }
}

/// Full sweep across every collection linked to the campaign. Abortable.
#[tauri::command]
pub async fn extract_all_from_campaign(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
) -> Result<ExtractionSummary, String> {
    let state_ref = state.inner().clone();
    let llm = state_ref.llm_provider.read().map_err(|e| format!("LLM lock: {e}"))?.clone();
    let embed = state_ref.embedding_provider.read().map_err(|e| format!("Embed lock: {e}"))?.clone();
    let app = app_handle.clone();

    let task = tokio::spawn(async move {
        let cids = crate::services::agent_service::resolve_collection_ids(&state_ref.db, &campaign_id)
            .await
            .map_err(|e| e.to_string())?;

        let mut entities_created = 0usize;
        let mut relations_created = 0usize;
        for cid in cids {
            let app = app.clone();
            let ec = entities_created;
            let rc = relations_created;
            let result = crate::services::extraction_service::extract_from_collection(
                &state_ref.db,
                &llm,
                &embed,
                &cid,
                move |mut p| {
                    // Make counts cumulative across collections.
                    p.entities_found += ec;
                    p.relations_found += rc;
                    emit_progress(&app, &p);
                },
            )
            .await
            .map_err(|e| e.to_string())?;
            entities_created += result.entities_created;
            relations_created += result.relations_created;
        }
        Ok::<_, String>((entities_created, relations_created))
    });

    *state.extract_task.lock().await = Some(task.abort_handle());

    match task.await {
        Ok(Ok((entities_created, relations_created))) => Ok(ExtractionSummary {
            entities_created,
            relations_created,
        }),
        Ok(Err(e)) => Err(format!("Extraction failed: {e}")),
        Err(join_err) if join_err.is_cancelled() => Err("cancelled".to_string()),
        Err(join_err) => Err(format!("Extraction task error: {join_err}")),
    }
}

/// Cancel the in-flight extraction task, if any.
#[tauri::command]
pub async fn cancel_extraction(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    crate::commands::cancel_chat_task(&state.extract_task).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn cancel_extraction_aborts_registered_task_and_empties_slot() {
        let slot: tokio::sync::Mutex<Option<tokio::task::AbortHandle>> =
            tokio::sync::Mutex::new(None);
        let task = tokio::spawn(async { loop { tokio::task::yield_now().await; } });
        *slot.lock().await = Some(task.abort_handle());

        assert!(crate::commands::cancel_chat_task(&slot).await);
        let err = task.await.expect_err("task should have been aborted");
        assert!(err.is_cancelled());
        assert!(!crate::commands::cancel_chat_task(&slot).await);
    }
}
```

- [ ] **Step 4: Update command registration in `lib.rs`**

In the `tauri::generate_handler![...]` list (near line 210), remove `commands::extract_entities_from_collection` and add:

```rust
            commands::extract_entity_by_name,
            commands::extract_all_from_campaign,
            commands::cancel_extraction,
```

Then check `src-tauri/src/commands/mod.rs` re-exports: if it has `pub use extraction_commands::extract_entities_from_collection;` (or a glob), update it to export the three new command functions instead. Search:

Run: `grep -n "extract_entities_from_collection\|extraction_commands" src-tauri/src/commands/mod.rs`
Replace any explicit re-export of the old name with the three new names (or leave a glob `pub use extraction_commands::*;` as-is).

- [ ] **Step 5: Build + test**

Run: `cargo build --manifest-path src-tauri/Cargo.toml && cargo test --manifest-path src-tauri/Cargo.toml cancel_extraction_aborts && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
Expected: builds, test PASSES, no clippy warnings.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/extraction_commands.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: extract_entity_by_name / extract_all_from_campaign / cancel_extraction commands"
```

---

## Task 6: Slash-command parser (frontend)

**Files:**
- Create: `src/lib/chat-commands.ts`
- Create: `src/lib/chat-commands.test.ts`

- [ ] **Step 1: Write the failing tests**

Create `src/lib/chat-commands.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { parseCommand } from './chat-commands';

describe('parseCommand', () => {
  it('parses /extract with a name', () => {
    expect(parseCommand('/extract Commander Varn')).toEqual({
      kind: 'extract',
      name: 'Commander Varn',
    });
  });

  it('trims surrounding whitespace from the name', () => {
    expect(parseCommand('  /extract   Iron Fist  ')).toEqual({
      kind: 'extract',
      name: 'Iron Fist',
    });
  });

  it('treats bare /extract as a usage hint, not a sweep', () => {
    expect(parseCommand('/extract')).toEqual({ kind: 'extract-usage' });
    expect(parseCommand('/extract   ')).toEqual({ kind: 'extract-usage' });
  });

  it('parses /extract-all', () => {
    expect(parseCommand('/extract-all')).toEqual({ kind: 'extract-all' });
  });

  it('parses /help', () => {
    expect(parseCommand('/help')).toEqual({ kind: 'help' });
  });

  it('treats unknown slash commands as help', () => {
    expect(parseCommand('/wat')).toEqual({ kind: 'help' });
  });

  it('passes normal text through as chat', () => {
    expect(parseCommand('How does grappling work?')).toEqual({
      kind: 'chat',
      text: 'How does grappling work?',
    });
  });

  it('does not treat a mid-sentence slash as a command', () => {
    expect(parseCommand('damage is 1/2')).toEqual({ kind: 'chat', text: 'damage is 1/2' });
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm test --run src/lib/chat-commands.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement the parser**

Create `src/lib/chat-commands.ts`:

```ts
export type ChatCommand =
  | { kind: 'extract'; name: string }
  | { kind: 'extract-all' }
  | { kind: 'extract-usage' }
  | { kind: 'help' }
  | { kind: 'chat'; text: string };

/**
 * Classify chat input. Only a leading slash on the first non-space character
 * is treated as a command, so "1/2" stays normal chat.
 */
export function parseCommand(raw: string): ChatCommand {
  const text = raw.trim();
  if (!text.startsWith('/')) {
    return { kind: 'chat', text };
  }

  const spaceIdx = text.indexOf(' ');
  const head = (spaceIdx === -1 ? text : text.slice(0, spaceIdx)).toLowerCase();
  const rest = spaceIdx === -1 ? '' : text.slice(spaceIdx + 1).trim();

  switch (head) {
    case '/extract':
      return rest ? { kind: 'extract', name: rest } : { kind: 'extract-usage' };
    case '/extract-all':
      return { kind: 'extract-all' };
    case '/help':
      return { kind: 'help' };
    default:
      return { kind: 'help' };
  }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `pnpm test --run src/lib/chat-commands.test.ts`
Expected: PASS (8 tests).

- [ ] **Step 5: Commit**

```bash
git add src/lib/chat-commands.ts src/lib/chat-commands.test.ts
git commit -m "feat: slash-command parser for chat"
```

---

## Task 7: Frontend command bindings (frontend)

**Files:**
- Modify: `src/lib/commands.ts:529-551` (the Entity Extraction section)

- [ ] **Step 1: Replace the Entity Extraction section**

Replace lines 529–551 (the `ExtractionSummary` / `ExtractionProgress` / `extractEntitiesFromCollection` block) with:

```ts
// ── Entity Extraction ────────────────────────────────────────────────────────

export interface ExtractionSummary {
  entities_created: number;
  relations_created: number;
}

export type ExtractionPhase =
  | 'resolving'
  | 'searching'
  | 'extracting'
  | 'relating'
  | 'embedding'
  | 'done'
  | 'empty';

export interface ExtractionProgress {
  phase: ExtractionPhase;
  detail: string;
  entities_found: number;
  relations_found: number;
}

/**
 * Seed-anchored extraction of a single named entity. Progress arrives via the
 * `extract-progress` Tauri event.
 */
export async function extractEntityByName(
  campaignId: string,
  name: string,
): Promise<ExtractionSummary> {
  return invoke<ExtractionSummary>('extract_entity_by_name', { campaignId, name });
}

/** Full sweep across all collections linked to the campaign. Cancellable. */
export async function extractAllFromCampaign(campaignId: string): Promise<ExtractionSummary> {
  return invoke<ExtractionSummary>('extract_all_from_campaign', { campaignId });
}

/** Abort the in-flight extraction, if any. */
export async function cancelExtraction(): Promise<void> {
  return invoke('cancel_extraction');
}
```

- [ ] **Step 2: Typecheck (expect breakage in CampaignView — fixed in Task 10)**

Run: `pnpm typecheck`
Expected: FAIL only in `src/views/CampaignView.svelte` referencing the removed `extractEntitiesFromCollection`. No other errors. (Task 10 removes that usage; if you prefer green between tasks, do Task 10 before this step — order is flexible since they are independent.)

- [ ] **Step 3: Commit**

```bash
git add src/lib/commands.ts
git commit -m "feat: frontend bindings for new extraction commands"
```

---

## Task 8: ExtractionCard component (frontend)

**Files:**
- Create: `src/components/ExtractionCard.svelte`
- Create: `src/components/ExtractionCard.test.ts`

- [ ] **Step 1: Write the failing tests**

Create `src/components/ExtractionCard.test.ts`:

```ts
import { render } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import ExtractionCard from './ExtractionCard.svelte';

describe('ExtractionCard', () => {
  it('shows the current phase detail while running', () => {
    const { getByText, getByRole } = render(ExtractionCard, {
      props: {
        status: 'running',
        title: 'Extracting "Commander Varn"',
        detail: 'Found 12 passages',
        entitiesFound: 0,
        relationsFound: 0,
        onCancel: () => {},
      },
    });
    expect(getByText('Found 12 passages')).toBeTruthy();
    expect(getByRole('button', { name: /cancel/i })).toBeTruthy();
  });

  it('shows a result summary on success and hides cancel', () => {
    const { getByText, queryByRole } = render(ExtractionCard, {
      props: {
        status: 'done',
        title: 'Extraction complete',
        detail: 'Created 5 entities, 4 relations',
        entitiesFound: 5,
        relationsFound: 4,
        onCancel: () => {},
      },
    });
    expect(getByText('Created 5 entities, 4 relations')).toBeTruthy();
    expect(queryByRole('button', { name: /cancel/i })).toBeNull();
  });

  it('renders the cancelled terminal state with kept counts', () => {
    const { getByText } = render(ExtractionCard, {
      props: {
        status: 'cancelled',
        title: 'Cancelled',
        detail: 'Cancelled — kept 2 entities / 1 relations created so far',
        entitiesFound: 2,
        relationsFound: 1,
        onCancel: () => {},
      },
    });
    expect(getByText(/kept 2 entities/)).toBeTruthy();
  });

  it('renders the empty terminal state', () => {
    const { getByText } = render(ExtractionCard, {
      props: {
        status: 'empty',
        title: 'Nothing found',
        detail: 'No passages found for "Ghost"',
        entitiesFound: 0,
        relationsFound: 0,
        onCancel: () => {},
      },
    });
    expect(getByText('No passages found for "Ghost"')).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm test --run src/components/ExtractionCard.test.ts`
Expected: FAIL — component not found.

- [ ] **Step 3: Implement the component**

Create `src/components/ExtractionCard.svelte`:

```svelte
<script lang="ts">
  type Status = 'running' | 'done' | 'empty' | 'cancelled' | 'error';

  let {
    status,
    title,
    detail,
    entitiesFound,
    relationsFound,
    onCancel,
  }: {
    status: Status;
    title: string;
    detail: string;
    entitiesFound: number;
    relationsFound: number;
    onCancel: () => void;
  } = $props();
</script>

<div class="extract-card" class:running={status === 'running'} role="status" aria-live="polite">
  <div class="head">
    {#if status === 'running'}
      <span class="spinner" aria-hidden="true"></span>
    {/if}
    <span class="title">{title}</span>
    {#if status === 'running'}
      <button class="btn-cancel" onclick={onCancel}>Cancel</button>
    {/if}
  </div>

  <p class="detail">{detail}</p>

  {#if entitiesFound > 0 || relationsFound > 0}
    <p class="counts">{entitiesFound} entities · {relationsFound} relations</p>
  {/if}
</div>

<style>
  .extract-card {
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--bg-panel-2);
    padding: 12px 14px;
    margin: 8px 0;
  }
  .head { display: flex; align-items: center; gap: 8px; }
  .title { font-weight: 600; color: var(--fg-1); flex: 1; }
  .detail { margin: 6px 0 0; color: var(--fg-2); font-size: 0.9rem; }
  .counts { margin: 4px 0 0; color: var(--fg-3); font-size: 0.8rem; }
  .btn-cancel {
    background: transparent; color: var(--fg-3);
    border: 1px solid var(--line); border-radius: 6px;
    padding: 4px 10px; cursor: pointer; font-size: 0.8rem;
  }
  .spinner {
    width: 12px; height: 12px; border-radius: 50%;
    border: 2px solid var(--line); border-top-color: var(--violet-300);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
```

- [ ] **Step 4: Run to verify pass**

Run: `pnpm test --run src/components/ExtractionCard.test.ts`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src/components/ExtractionCard.svelte src/components/ExtractionCard.test.ts
git commit -m "feat: ExtractionCard component"
```

---

## Task 9: Wire commands + card into OracleView (frontend)

**Files:**
- Modify: `src/views/OracleView.svelte`
- Modify: `src/views/OracleView.test.ts`

- [ ] **Step 1: Write the failing routing tests**

Add to `src/views/OracleView.test.ts` (follow the existing mock setup in that file — it already mocks `../lib/commands` and `@tauri-apps/api/event`; extend the mock to include the new functions). Add:

```ts
it('routes /extract <name> to extractEntityByName, not chatSend', async () => {
  const { extractEntityByName, chatSend } = await import('../lib/commands');
  // render OracleView with activeCampaignId set, type "/extract Varn", submit.
  // (Use the same render+input helper the other tests in this file use.)
  // Assert:
  expect(extractEntityByName).toHaveBeenCalledWith(expect.any(String), 'Varn');
  expect(chatSend).not.toHaveBeenCalled();
});

it('bare /extract shows a usage hint and starts no extraction', async () => {
  const { extractEntityByName, extractAllFromCampaign, chatSend } = await import('../lib/commands');
  // type "/extract", submit.
  expect(extractEntityByName).not.toHaveBeenCalled();
  expect(extractAllFromCampaign).not.toHaveBeenCalled();
  expect(chatSend).not.toHaveBeenCalled();
  // a usage message is shown in the thread (assert on the rendered hint text).
});
```

> Implementation note for the worker: mirror the existing test in `OracleView.test.ts` that drives the textarea and submit button. Reuse its `vi.mock('../lib/commands', ...)` block and add `extractEntityByName: vi.fn().mockResolvedValue({ entities_created: 0, relations_created: 0 })`, `extractAllFromCampaign: vi.fn().mockResolvedValue({ entities_created: 0, relations_created: 0 })`, and `cancelExtraction: vi.fn()`.

- [ ] **Step 2: Run to verify failure**

Run: `pnpm test --run src/views/OracleView.test.ts`
Expected: FAIL — routing not implemented; new mocks referenced.

- [ ] **Step 3: Add imports and extraction state to OracleView**

In the `<script>` block, extend the `../lib/commands` import (around line 4) to add:

```ts
    extractEntityByName,
    extractAllFromCampaign,
    cancelExtraction,
    type ExtractionProgress,
```

Add `parseCommand`:

```ts
  import { parseCommand } from '../lib/chat-commands';
  import ExtractionCard from '../components/ExtractionCard.svelte';
```

Add state (near the other `$state` declarations, ~line 36):

```ts
  type ExtractionStatus = 'running' | 'done' | 'empty' | 'cancelled' | 'error';
  let extraction = $state<{
    status: ExtractionStatus;
    title: string;
    detail: string;
    entitiesFound: number;
    relationsFound: number;
  } | null>(null);
  let unlistenExtract: UnlistenFn | null = null;
```

- [ ] **Step 4: Listen for `extract-progress` in `onMount` and clean up in `onDestroy`**

Inside `onMount`, after the existing `chat-token` listener, add:

```ts
    unlistenExtract = await listen<ExtractionProgress>('extract-progress', (event) => {
      const p = event.payload;
      if (!extraction) return;
      extraction = {
        ...extraction,
        detail: p.detail,
        entitiesFound: p.entities_found,
        relationsFound: p.relations_found,
        status: p.phase === 'done' ? 'done' : p.phase === 'empty' ? 'empty' : 'running',
      };
    });
```

In `onDestroy`, add:

```ts
    if (unlistenExtract) unlistenExtract();
```

- [ ] **Step 5: Add the command handlers and route them in `sendMessage`**

Replace the body of `sendMessage` (lines 136–154) so it routes through `parseCommand`:

```ts
  async function sendMessage(text?: string) {
    const t = (text ?? input).trim();
    if (!t || isLoading || extraction?.status === 'running') return;

    const cmd = parseCommand(t);
    if (cmd.kind !== 'chat') {
      input = '';
      if (inputEl) { inputEl.style.height = 'auto'; inputEl.focus(); }
      handleCommand(cmd);
      return;
    }

    messages = [...messages, { role: 'user', content: t }];
    input = '';
    if (inputEl) { inputEl.style.height = 'auto'; inputEl.focus(); }
    atBottom = true;
    isLoading = true;
    currentResponse = '';
    try {
      await chatSend(t, activeCampaignId);
    } catch (e) {
      messages = [...messages, { role: 'error', content: String(e) }];
      isLoading = false;
    }
  }

  function handleCommand(cmd: ReturnType<typeof parseCommand>) {
    switch (cmd.kind) {
      case 'extract-usage':
        messages = [...messages, {
          role: 'system',
          content: 'Usage: /extract <entity name>. To extract everything from all books, use /extract-all (this can take a while).',
        }];
        return;
      case 'help':
        messages = [...messages, {
          role: 'system',
          content: 'Commands: /extract <name> — build one entity; /extract-all — extract everything (slow); /help — this list.',
        }];
        return;
      case 'extract':
        runExtraction(() => extractEntityByName(activeCampaignId ?? '', cmd.name), `Extracting "${cmd.name}"`);
        return;
      case 'extract-all':
        runExtraction(() => extractAllFromCampaign(activeCampaignId ?? ''), 'Extracting all entities');
        return;
    }
  }

  async function runExtraction(start: () => Promise<{ entities_created: number; relations_created: number }>, title: string) {
    if (!activeCampaignId) {
      messages = [...messages, { role: 'error', content: 'Select a campaign first.' }];
      return;
    }
    extraction = { status: 'running', title, detail: 'Starting…', entitiesFound: 0, relationsFound: 0 };
    try {
      const summary = await start();
      extraction = {
        status: extraction?.status === 'empty' ? 'empty' : 'done',
        title: 'Extraction complete',
        detail: extraction?.status === 'empty'
          ? extraction.detail
          : `Created ${summary.entities_created} entities, ${summary.relations_created} relations`,
        entitiesFound: summary.entities_created,
        relationsFound: summary.relations_created,
      };
    } catch (e) {
      const cancelled = String(e).includes('cancelled');
      extraction = {
        status: cancelled ? 'cancelled' : 'error',
        title: cancelled ? 'Cancelled' : 'Extraction failed',
        detail: cancelled
          ? `Cancelled — kept ${extraction?.entitiesFound ?? 0} entities / ${extraction?.relationsFound ?? 0} relations created so far`
          : String(e),
        entitiesFound: extraction?.entitiesFound ?? 0,
        relationsFound: extraction?.relationsFound ?? 0,
      };
    }
  }

  async function cancelActiveExtraction() {
    try {
      await cancelExtraction();
    } catch (e) {
      console.error('Failed to cancel extraction:', e);
    }
  }
```

- [ ] **Step 6: Render the card in the template**

In the message-thread markup, after the streaming-response block (where `currentResponse`/`isLoading` render), add:

```svelte
  {#if extraction}
    <ExtractionCard
      status={extraction.status}
      title={extraction.title}
      detail={extraction.detail}
      entitiesFound={extraction.entitiesFound}
      relationsFound={extraction.relationsFound}
      onCancel={cancelActiveExtraction}
    />
  {/if}
```

Also ensure `system`-role messages render as an info line in the existing `{#each messages}` loop (add a branch: if `message.role === 'system'`, render `<p class="system-note">{message.content}</p>`). Add a minimal style: `.system-note { color: var(--fg-3); font-size: 0.85rem; font-style: italic; }`.

- [ ] **Step 7: Run the tests**

Run: `pnpm test --run src/views/OracleView.test.ts`
Expected: PASS — existing tests plus the two new routing tests.

- [ ] **Step 8: Commit**

```bash
git add src/views/OracleView.svelte src/views/OracleView.test.ts
git commit -m "feat: route /extract commands and render progress card in OracleView"
```

---

## Task 10: Remove the old extract button (frontend)

**Files:**
- Modify: `src/views/CampaignView.svelte` (remove lines ~14, ~18, 117–140, 381–397, 791–801 per the current file)

- [ ] **Step 1: Remove the extraction code from CampaignView**

In `src/views/CampaignView.svelte`:
- Remove the `extractEntitiesFromCollection` import (line 14) and the `ExtractionProgress` type import (line 18).
- Remove the `// ── Extraction ──` state block (`extractingColId`, `extractionProgress`, `extractionToast`) and the `runExtraction` function (lines ~117–140).
- Remove the extract `<button class="add-book extract-btn" …>` and the `{#if extractionToast …}` block in the template (lines ~381–397).
- Remove the `.extract-btn`, `.extract-btn:disabled`, and `.extract-toast` style rules (lines ~791–801).

Use grep to confirm nothing remains:

Run: `grep -n "extract\|Extract" src/views/CampaignView.svelte`
Expected: no matches.

- [ ] **Step 2: Typecheck, lint, and run the frontend suite**

Run: `pnpm typecheck && pnpm lint && pnpm test --run`
Expected: PASS, no type errors (the removed binding is no longer referenced anywhere).

- [ ] **Step 3: Commit**

```bash
git add src/views/CampaignView.svelte
git commit -m "refactor: remove per-collection extract button (moved to chat /extract)"
```

---

## Task 11: Full verification

- [ ] **Step 1: Backend**

Run: `cargo fmt --manifest-path src-tauri/Cargo.toml && cargo test --manifest-path src-tauri/Cargo.toml && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`
Expected: formatted, all tests PASS, no warnings.

- [ ] **Step 2: Frontend**

Run: `pnpm typecheck && pnpm lint && pnpm test --run`
Expected: all PASS.

- [ ] **Step 3: Final commit (if fmt changed anything)**

```bash
git add -A && git commit -m "chore: fmt after /extract feature" || echo "nothing to commit"
```

---

## Self-Review Notes (addressed)

- **Spec coverage:** `/extract <name>` (Task 3, 6, 9) · `/extract` usage hint (Task 6, 9) · `/extract-all` (Task 5, 9) · `/help` + unknown (Task 6, 9) · live phase card (Task 8, 9) · cancel with kept counts (Task 5, 9) · empty/failure states (Task 8, 9) · per-collection scoped storage (Task 3) · button removal (Task 10) · phased event payload (Task 1, 7).
- **Future work (`/extract-all` revisit)** is intentionally out of scope — implemented as the placeholder full sweep per the spec.
- **Type consistency:** `ExtractionPhase`/`ExtractionProgress` match across Rust (Task 1) and TS (Task 7); `extractEntityByName(campaignId, name)` signature matches its call site (Task 9) and binding (Task 7); `cancel_chat_task` reused for the extract slot (Task 5).
