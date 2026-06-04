# Hybrid Entity Retrieval Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Inject campaign entity records into the Oracle's LLM context so queries like "who are Nico's characters?" return answers from the GM's own notes alongside PDF-sourced rules.

**Architecture:** New `fetch_entity_context` function queries all 8 entity tables at chat time and injects them as a `CAMPAIGN NOTES:` block in the system prompt. `build_rag_system_prompt` is renamed to `build_system_prompt(rag, entities)` to hold both contexts. Frontend `renderContent` gains an `ENTITY_RE` pass that renders `[Entity: "name", kind: "kind"]` markers as violet inline badge spans. No schema changes, no embedding at write time.

**Tech Stack:** Rust/SurrealDB (`agent_service.rs`), TypeScript (`ruling-parse.ts`), Svelte 5 (`OracleView.svelte`), Vitest, `#[tokio::test]`.

**Spec:** `docs/superpowers/specs/2026-06-04-hybrid-entity-retrieval-design.md`

---

## File Map

| File | Change |
|------|--------|
| `src-tauri/src/services/agent_service.rs` | Add `fetch_entity_context`; rename `build_rag_system_prompt` → `build_system_prompt(rag, entities)`; update `stream_response` call site + 3 existing tests |
| `src/views/ruling-parse.ts` | Add `ENTITY_RE` const; update `renderContent` to chain a second `.replace` |
| `src/views/OracleView.svelte` | Add `:global(.entity-badge)` CSS block after existing `:global(.citation-badge)` |

---

## Task 1: `fetch_entity_context` — query all entity tables and format context string

**Files:**
- Modify: `src-tauri/src/services/agent_service.rs`

**Context:** `agent_service.rs` already imports `surrealdb::Connection`. All 8 entity tables (`npc`, `location`, `faction`, `creature`, `item`, `event`, `player_character`, `misc`) were created in migration `004_graph_entities.surql`. `crate::schema::run_migrations(&db)` sets them all up in tests. The tests block is `mod tests` at line ~389.

- [ ] **Step 1: Write four failing tests**

Add these four tests inside the existing `mod tests { ... }` block at the bottom of `src-tauri/src/services/agent_service.rs`:

```rust
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

    let result = fetch_entity_context(&db, "camp1").await.unwrap();
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
         campaign=type::thing('campaign','camp1'), \
         name='Nazirdijan', player_name='Nico', character_class='Wizard', \
         character_level=5, status='active', summary=NULL, notes=NULL, \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();

    let result = fetch_entity_context(&db, "camp1").await.unwrap();
    assert!(result.contains("[player_character] Nazirdijan"), "missing entity line: {result}");
    assert!(result.contains("Player: Nico"), "missing player_name: {result}");
    assert!(result.contains("Class: Wizard"), "missing class: {result}");
    assert!(result.contains("Level: 5"), "missing level: {result}");
    assert!(result.contains("Status: active"), "missing status: {result}");
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
        "CREATE npc SET id='npc1', campaign=type::thing('campaign','camp1'), \
         name='Aldric the Smith', summary='village blacksmith', notes=NULL, \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();

    let result = fetch_entity_context(&db, "camp1").await.unwrap();
    assert!(result.contains("[npc] Aldric the Smith"), "missing npc: {result}");
    assert!(result.contains("village blacksmith"), "missing summary: {result}");
    assert!(!result.contains("[player_character]"), "unexpected PC section: {result}");
    assert!(!result.contains("[location]"), "unexpected location section: {result}");
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
        "CREATE event SET id='ev1', campaign=type::thing('campaign','camp1'), \
         name='Battle of Irongate', date_start='Year 312', date_end='Year 313', \
         summary=NULL, notes=NULL, is_ongoing=false, \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();

    let result = fetch_entity_context(&db, "camp1").await.unwrap();
    assert!(result.contains("[event] Battle of Irongate"), "missing event: {result}");
    assert!(result.contains("Year 312 → Year 313"), "missing dates: {result}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test fetch_entity_context -- --nocapture
```

Expected: FAIL — `fetch_entity_context` is not defined.

- [ ] **Step 3: Implement `fetch_entity_context`**

Add this function to `src-tauri/src/services/agent_service.rs` immediately after `resolve_collection_ids` (around line 61, before `stream_response`):

```rust
/// Query all entity tables for a campaign and format them as a context block.
///
/// Returns an empty string when the campaign has no entities — callers use this
/// to skip the CAMPAIGN NOTES section entirely.
pub async fn fetch_entity_context<C: Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
) -> Result<String, AgentError> {
    #[derive(serde::Deserialize)]
    struct BasicRow {
        name: String,
        summary: Option<String>,
    }

    #[derive(serde::Deserialize)]
    struct PcRow {
        name: String,
        summary: Option<String>,
        player_name: Option<String>,
        character_class: Option<String>,
        character_level: Option<i64>,
        status: Option<String>,
    }

    #[derive(serde::Deserialize)]
    struct EventRow {
        name: String,
        summary: Option<String>,
        date_start: Option<String>,
        date_end: Option<String>,
    }

    let mut resp = db
        .query("SELECT name, summary, player_name, character_class, character_level, status FROM player_character WHERE campaign = type::thing('campaign', $cid) ORDER BY name ASC")
        .query("SELECT name, summary FROM npc WHERE campaign = type::thing('campaign', $cid) ORDER BY name ASC")
        .query("SELECT name, summary FROM location WHERE campaign = type::thing('campaign', $cid) ORDER BY name ASC")
        .query("SELECT name, summary FROM faction WHERE campaign = type::thing('campaign', $cid) ORDER BY name ASC")
        .query("SELECT name, summary FROM creature WHERE campaign = type::thing('campaign', $cid) ORDER BY name ASC")
        .query("SELECT name, summary FROM item WHERE campaign = type::thing('campaign', $cid) ORDER BY name ASC")
        .query("SELECT name, summary, date_start, date_end FROM event WHERE campaign = type::thing('campaign', $cid) ORDER BY name ASC")
        .query("SELECT name, summary FROM misc WHERE campaign = type::thing('campaign', $cid) ORDER BY name ASC")
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

    if pcs.is_empty()
        && npcs.is_empty()
        && locations.is_empty()
        && factions.is_empty()
        && creatures.is_empty()
        && items.is_empty()
        && events.is_empty()
        && misc.is_empty()
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
                out.push('\n');
            }
        }
    }

    if !events.is_empty() {
        out.push('\n');
        for r in &events {
            out.push_str(&format!("[event] {}", r.name));
            match (&r.date_start, &r.date_end) {
                (Some(s), Some(e)) if !s.trim().is_empty() => {
                    out.push_str(&format!(" · {s} → {e}"));
                }
                (Some(s), None) if !s.trim().is_empty() => {
                    out.push_str(&format!(" · {s}"));
                }
                _ => {}
            }
            if let Some(s) = &r.summary {
                if !s.trim().is_empty() {
                    out.push_str(&format!(" · {s}"));
                }
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
            out.push('\n');
        }
    }

    Ok(out)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test fetch_entity_context -- --nocapture
```

Expected: 4 tests pass.

- [ ] **Step 5: Run the full test suite to check for regressions**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/services/agent_service.rs
git commit -m "feat: add fetch_entity_context to build campaign notes context"
```

---

## Task 2: Rename `build_rag_system_prompt` → `build_system_prompt` with two-arg signature

**Files:**
- Modify: `src-tauri/src/services/agent_service.rs`

**Context:** `build_rag_system_prompt` is called in three places: `stream_response` (line 113) and three tests (lines 428, 437, 457). The function is private (`fn`, no `pub`). The rename and signature change must happen atomically — update the function definition and all three call sites together in one edit pass.

- [ ] **Step 1: Write four failing tests for the new `build_system_prompt` signature**

Add these tests inside the existing `mod tests` block:

```rust
#[test]
fn build_system_prompt_both_contexts_includes_both_sections() {
    let rag = "[0] Source: \"PHB\", p.1 — \"Intro\"\nSome rules.\n\n";
    let ent = "Campaign notes (your GM records):\n\n[npc] Aldric\n";
    let prompt = build_system_prompt(rag, ent);
    assert!(prompt.contains("REFERENCE MATERIAL"), "missing RAG section");
    assert!(prompt.contains("CAMPAIGN NOTES"), "missing entity section");
    assert!(prompt.contains("[Entity:"), "missing entity citation instruction");
    assert!(prompt.contains("[Source:"), "missing source citation instruction");
}

#[test]
fn build_system_prompt_entity_only_omits_rag_section() {
    let ent = "Campaign notes (your GM records):\n\n[npc] Aldric\n";
    let prompt = build_system_prompt("", ent);
    assert!(prompt.contains("CAMPAIGN NOTES"), "missing entity section");
    assert!(!prompt.contains("REFERENCE MATERIAL"), "unexpected RAG section");
    assert!(prompt.contains("[Entity:"), "missing entity citation instruction");
    assert!(!prompt.contains("Entity scope is critical"), "unexpected RAG-only instruction");
}

#[test]
fn build_system_prompt_rag_only_regression() {
    // Regression: existing behaviour must be preserved when entity_context is empty.
    let rag = "[0] Source: \"PHB\", p.1 — \"Intro\"\nSome rules.\n\n";
    let prompt = build_system_prompt(rag, "");
    assert!(prompt.contains("REFERENCE MATERIAL"), "missing RAG section");
    assert!(!prompt.contains("CAMPAIGN NOTES"), "unexpected entity section");
    assert!(prompt.contains("Entity scope is critical"), "missing scope guard");
    assert!(prompt.contains("SEPARATE entities"), "missing entity contamination guard");
    assert!(prompt.contains("list / enumeration"), "missing enumeration instruction");
    assert!(prompt.contains("Do not compress"), "missing list-compression guard");
}

#[test]
fn build_system_prompt_neither_returns_fallback() {
    let prompt = build_system_prompt("", "");
    assert!(!prompt.contains("REFERENCE MATERIAL"), "unexpected RAG section");
    assert!(!prompt.contains("CAMPAIGN NOTES"), "unexpected entity section");
    assert!(prompt.contains("Game Master assistant"), "missing base identity");
}
```

- [ ] **Step 2: Run new tests to verify they fail**

```bash
cargo test "build_system_prompt_both\|build_system_prompt_entity_only\|build_system_prompt_rag_only_regression\|build_system_prompt_neither" -- --nocapture
```

Expected: FAIL — `build_system_prompt` not defined.

- [ ] **Step 3: Replace `build_rag_system_prompt` with `build_system_prompt`**

Replace the existing `build_rag_system_prompt` function (around line 196 in `agent_service.rs`) with:

```rust
fn build_system_prompt(rag_context: &str, entity_context: &str) -> String {
    let has_rag = !rag_context.is_empty();
    let has_entities = !entity_context.is_empty();

    if !has_rag && !has_entities {
        return "You are an expert Game Master assistant. \
            Answer the user's question to the best of your ability. \
            If you don't know the answer, say so — do not make up rules."
            .to_string();
    }

    let mut prompt = String::from("You are an expert Game Master assistant.\n\n");

    if has_rag {
        prompt.push_str(&format!("REFERENCE MATERIAL:\n{rag_context}\n"));
    }

    if has_entities {
        prompt.push_str(&format!("CAMPAIGN NOTES (GM's own records):\n{entity_context}\n"));
    }

    prompt.push_str("INSTRUCTIONS:\n");
    prompt.push_str("- Read every passage and note above carefully BEFORE answering.\n");

    if has_rag {
        prompt.push_str(
            "- Entity scope is critical. A passage is valid evidence ONLY when it \
             explicitly attributes a fact to the SAME entity the question is about. \
             A passage that lists the target entity alongside OTHER entities \
             (e.g. \"X dominates Vethara, Korim, Suthen and Marrowen\", or \
             \"in Vethara and in Korim\") does NOT attribute everything in the \
             list to the target — those are SEPARATE entities. Wording can vary \
             (synonyms and paraphrases are fine when they refer to the same entity, \
             e.g. \"factions\" ≈ \"groups\"), but a fact about a different but \
             co-mentioned entity is NOT evidence for the target.\n\
             - For list / enumeration questions (\"which are the...\", \"what are the...\", \
             \"list...\"), enumerate EVERY item the passages explicitly attribute to \
             the target entity. Do not compress to fit a sentence budget. If the \
             passages only cover some items, list those and acknowledge that the \
             reference material may be incomplete.\n",
        );
    }

    prompt.push_str(
        "- For other questions, answer in 1–3 sentences. Be concise — the GM is \
         running a table.\n\
         - Do NOT quote the passages verbatim in your answer text — the supporting \
         quote belongs INSIDE the citation marker.\n",
    );

    if has_rag {
        prompt.push_str(
            "- Every factual claim from REFERENCE MATERIAL must cite its source using \
             this exact format, including a short verbatim quote (1 sentence) from the \
             passage that supports the claim:\n  \
               [Source: \"<source name>\", p.<page>, quote: \"<verbatim sentence>\"]\n  \
             Example: [Source: \"PHB\", p.72, quote: \"A fighter can use Action Surge once per rest.\"]\n  \
             The UI hides the quote from the visible reply and shows it in a popover \
             when the user clicks the citation badge.\n\
             - Only say \"the reference material does not contain this information\" if you \
             have scanned every passage and found no relevant content (paraphrase counts \
             only for the same entity).\n",
        );
    }

    if has_entities {
        prompt.push_str(
            "- Every factual claim from CAMPAIGN NOTES must cite the entity using \
             this exact format:\n  \
               [Entity: \"<entity name>\", kind: \"<kind>\"]\n  \
             where kind is the bracketed prefix in the campaign note line \
             (e.g. player_character, npc, location, faction, creature, item, event, misc).\n  \
             Example: [Entity: \"Nazirdijan\", kind: \"player_character\"]\n  \
             No verbatim quote is needed — entity records are the GM's own data.\n\
             - Entity names in CAMPAIGN NOTES are exact — use them verbatim in citations.\n",
        );
    }

    prompt
}
```

- [ ] **Step 4: Update the three existing call sites in the test block**

In `mod tests`, find the three calls to `build_rag_system_prompt` and change each to `build_system_prompt(arg, "")`:

```rust
// was: let prompt = build_rag_system_prompt("");
let prompt = build_system_prompt("", "");

// was: let prompt = build_rag_system_prompt(ctx);
let prompt = build_system_prompt(ctx, "");

// was: let prompt = build_rag_system_prompt("[0] Source: \"x.pdf\", p. 1 — \"\"\ntext\n\n");
let prompt = build_system_prompt("[0] Source: \"x.pdf\", p. 1 — \"\"\ntext\n\n", "");
```

- [ ] **Step 5: Run all tests to verify everything passes**

```bash
cargo test -- --nocapture 2>&1 | tail -20
```

Expected: all tests pass. The renamed function satisfies both new tests and the three updated existing tests.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/services/agent_service.rs
git commit -m "refactor: rename build_rag_system_prompt to build_system_prompt with entity context arg"
```

---

## Task 3: Wire `fetch_entity_context` into `stream_response`

**Files:**
- Modify: `src-tauri/src/services/agent_service.rs`

**Context:** `stream_response` is the function that runs the full pipeline. It currently calls `build_rag_system_prompt(&context)` at line 113. The change adds a single `fetch_entity_context` call after `resolve_collection_ids` and threads the result into `build_system_prompt`. No new tests are needed here — the behaviour is covered by the unit tests in Tasks 1 and 2. The `CHRONACLE_RAG_DEBUG` block that logs the system prompt will automatically log both sections once wired.

- [ ] **Step 1: Add `entity_context` fetch after `resolve_collection_ids` in `stream_response`**

In `stream_response`, after these lines (around line 91–102):

```rust
    let collection_ids = match campaign_id {
        Some(cid) => resolve_collection_ids(&state.db, cid)
            .await
            .map_err(|e| AgentError::Retrieval(e.to_string()))?,
        None => Vec::new(),
    };
```

Add immediately after:

```rust
    let entity_context = match campaign_id {
        Some(cid) => fetch_entity_context(&state.db, cid)
            .await
            .unwrap_or_else(|e| {
                eprintln!("entity context fetch failed: {e}");
                String::new()
            }),
        None => String::new(),
    };
```

- [ ] **Step 2: Update the `build_rag_system_prompt` call to `build_system_prompt`**

Change line ~113:

```rust
    // was:
    let system_prompt = build_rag_system_prompt(&context);
    // becomes:
    let system_prompt = build_system_prompt(&context, &entity_context);
```

- [ ] **Step 3: Build to verify no compile errors**

```bash
cargo build 2>&1 | grep -E "^error"
```

Expected: no output (clean build).

- [ ] **Step 4: Run full test suite**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/agent_service.rs
git commit -m "feat: inject entity context into Oracle RAG pipeline"
```

---

## Task 4: Frontend — add `ENTITY_RE` and update `renderContent` in `ruling-parse.ts`

**Files:**
- Modify: `src/views/ruling-parse.ts`

**Context:** `renderContent` (line 58) currently does a single `.replace(SOURCE_RE, ...)`. `SOURCE_RE` is defined at module level as a `/g` regex. A new module-level `ENTITY_RE` will match `[Entity: "name", kind: "kind"]`. The `renderContent` function chains a second `.replace`. Entity badges are `<span>` (non-clickable), not `<button>`. The existing tests in `src/views/ruling-parse.test.ts` test `renderContent` and must keep passing.

- [ ] **Step 1: Write three failing Vitest tests**

Add these tests to the existing `describe('renderContent', ...)` block in `src/views/ruling-parse.test.ts`:

```typescript
  it('replaces [Entity] with an entity-badge span', () => {
    const html = renderContent(
      'Nazirdijan acts [Entity: "Nazirdijan", kind: "player_character"].',
    );
    expect(html).toContain('<span class="entity-badge"');
    expect(html).toContain('title="player_character"');
    expect(html).toContain('>Nazirdijan<');
  });

  it('escapes a malicious entity name in [Entity]', () => {
    const html = renderContent('[Entity: "<script>alert(1)</script>", kind: "npc"]');
    expect(html).not.toMatch(/<script>/);
    expect(html).toContain('&lt;script&gt;');
  });

  it('renders both Source and Entity markers in the same string', () => {
    const html = renderContent(
      'Rules apply [Source: "PHB", p.72]. Nazirdijan agrees [Entity: "Nazirdijan", kind: "player_character"].',
    );
    expect(html).toContain('class="citation-badge"');
    expect(html).toContain('class="entity-badge"');
  });
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
pnpm test --run 2>&1 | grep -A3 "entity-badge\|FAIL\|✗"
```

Expected: 3 new tests fail — `entity-badge` span is not produced.

- [ ] **Step 3: Add `ENTITY_RE` constant and update `renderContent`**

In `src/views/ruling-parse.ts`, add the new constant immediately after the existing `SOURCE_RE` declaration:

```typescript
const ENTITY_RE = /\[Entity:\s*"([^"]+)",\s*kind:\s*"([^"]+)"\s*\]/g;
```

Then update `renderContent` to chain the second replace:

```typescript
export function renderContent(text: string): string {
  return text
    .replace(SOURCE_RE, (_, name: string, page: string | undefined, quote: string | undefined) => {
      const dataPage = page ? ` data-page="${escapeAttr(page)}"` : '';
      const dataQuote = quote ? ` data-quote="${escapeAttr(quote)}"` : '';
      const label = `${escapeAttr(name)}${page ? ` p.${escapeAttr(page)}` : ''}`;
      return `<button type="button" class="citation-badge" data-source="${escapeAttr(name)}"${dataPage}${dataQuote} title="Show source passage">${label}</button>`;
    })
    .replace(ENTITY_RE, (_, name: string, kind: string) =>
      `<span class="entity-badge" title="${escapeAttr(kind)}">${escapeAttr(name)}</span>`,
    );
}
```

- [ ] **Step 4: Run all frontend tests**

```bash
pnpm test --run
```

Expected: all tests pass, including the 3 new ones and the existing `renderContent` tests.

- [ ] **Step 5: Commit**

```bash
git add src/views/ruling-parse.ts src/views/ruling-parse.test.ts
git commit -m "feat: render [Entity] citation markers as entity-badge spans"
```

---

## Task 5: Add `.entity-badge` CSS to `OracleView.svelte`

**Files:**
- Modify: `src/views/OracleView.svelte`

**Context:** The existing `:global(.citation-badge)` rule is at line 550. Entity badges use violet (`--violet-300 = #b8a6ff`) to distinguish them from PDF citation badges which use arcane blue (`--arcane-300`). No JS changes needed — entity badges are non-clickable spans.

- [ ] **Step 1: Add the entity badge style after the existing citation badge rules**

In `src/views/OracleView.svelte`, find the `:global(.citation-badge:hover)` block (line ~564) and add immediately after it:

```css
  :global(.entity-badge) {
    display: inline-flex;
    align-items: baseline;
    padding: 1px 8px;
    border-radius: var(--r-full);
    border: 1px solid var(--line);
    color: var(--violet-300);
    background: rgba(184, 166, 255, 0.08);
    font-family: var(--font-mono);
    font-size: 12px;
    margin: 0 2px;
  }
```

- [ ] **Step 2: Typecheck and lint**

```bash
pnpm typecheck && pnpm lint
```

Expected: no errors.

- [ ] **Step 3: Run full frontend test suite one last time**

```bash
pnpm test --run
```

Expected: all tests pass.

- [ ] **Step 4: Run full Rust test suite one last time**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/views/OracleView.svelte
git commit -m "feat: add entity-badge pill style for Oracle entity citations"
```

---

## Verification

After all tasks complete:

1. `cargo tauri dev` from the project root.
2. Open a campaign, go to the Oracle.
3. Create a player character named "Nazirdijan" with player name "Nico" (if not already done).
4. Ask: **"Who are Nico's characters?"** — the Oracle should answer with Nazirdijan's name and an `[Entity: "Nazirdijan", kind: "player_character"]` badge rendered as a violet pill.
5. Ask a PDF rules question — verify `[Source: ...]` citation badges are unaffected.
6. Ask a mixed question, e.g., **"What class is Nazirdijan and what do Wizards get at level 5?"** — verify both violet entity badge and arcane PDF badge appear in the response.
7. (Optional) Set `CHRONACLE_RAG_DEBUG=1` and restart — verify the system prompt printed to stderr contains both `REFERENCE MATERIAL` and `CAMPAIGN NOTES` sections when a campaign is active.
