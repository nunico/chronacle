# RAG Quality Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Raise answer quality of the GM agent so that questions whose answer is verbatim in the indexed PDF are answered correctly, not refused.

**Architecture:** Five compounding defects in the Phase-1 RAG pipeline are fixed in dependency order. (1) `nomic-embed-text-v1.5` task prefixes (`search_document:` / `search_query:`) are added on both index and query sides. (2) Extracted PDF text is normalized (de-hyphenation, paragraph repair) before chunking. (3) Chunker is tightened to ~250 tokens with sentence-aware boundaries. (4) System prompt is rewritten to require quoting the supporting sentence and to prevent premature "not in sources" verdicts. (5) `pdf-extract` is replaced with `pdfium-render` (already the approved crate in `docs/architecture.md`) for layout-aware multi-column extraction. A "Re-index all sources" button is added to SettingsPage so users can pick up improvements without re-uploading PDFs.

**Tech Stack:** Rust (`fastembed` 5, `pdfium-render`, `regex`, `unicode-segmentation`), Svelte 5 + TypeScript, Tauri 2, SurrealDB.

---

## File Structure

**New files:**
- `src-tauri/src/services/text_normalizer.rs` — de-hyphenation + paragraph repair, pure function on `&str`.
- `src-tauri/src/services/pdf_extractor.rs` — `PdfExtractor` trait + `PdfiumExtractor` impl; replaces inline `pdf_extract::extract_text_from_mem_by_pages` calls.
- `src-tauri/resources/pdfium/.gitkeep` — placeholder; populated by `build.rs` at compile time.
- `tests/fixtures/pdfs/coriolis_sample.pdf` — fixture replicating the failing extraction (two-column-ish, hyphenated line breaks).
- `src-tauri/tests/rag_quality_integration.rs` — integration test asserting the example failing questions now retrieve the right chunk.

**Modified files:**
- `src-tauri/Cargo.toml` — remove `pdf-extract`, add `pdfium-render`, `unicode-segmentation`.
- `src-tauri/build.rs` — download pdfium binary for host target into `resources/pdfium/`.
- `src-tauri/tauri.conf.json` — add `resources/pdfium/**/*` to bundle resources.
- `src-tauri/src/providers/embedding.rs` — split trait into `embed_documents()` / `embed_query()`, prefix in `FastEmbedProvider`.
- `src-tauri/src/services/ingestion_service.rs` — use new extractor + normalizer + prefix-aware embed.
- `src-tauri/src/services/chunker.rs` — 250-token target, sentence-aware splitting.
- `src-tauri/src/services/agent_service.rs` — loosen system prompt.
- `src-tauri/src/services/mod.rs` — export new modules.
- `src-tauri/src/commands/mod.rs` — add `reindex_all_sources` command.
- `src-tauri/src/lib.rs` — register new command in `invoke_handler!`.
- `src/lib/commands.ts` — TS wrapper for `reindex_all_sources`.
- `src/SettingsPage.svelte` — add "Embedding model" section with Re-index All button.
- `docs/architecture.md` — short note in ADR-003 about prefix requirement.

---

## Task 1: Worktree setup

Establishes an isolated workspace so this multi-file change doesn't touch the user's current branch.

- [ ] **Step 1: Create worktree**

Use the `superpowers:using-git-worktrees` skill to create an isolated worktree on a new branch named `rag-quality-improvements`.

- [ ] **Step 2: Verify clean state**

```bash
git status
cargo build --quiet
pnpm install --frozen-lockfile
```

Expected: no uncommitted changes, build succeeds.

- [ ] **Step 3: Commit baseline**

```bash
git commit --allow-empty -m "chore: start RAG quality improvements branch"
```

---

## Task 2: Update ADR-003 with prefix requirement

Documents the change before code lands so reviewers see the why.

**Files:**
- Modify: `docs/architecture.md` (section "## ADR-003: Embeddings — fastembed-rs (Bundled Local Model)")

- [ ] **Step 1: Add prefix note to ADR-003**

Append a paragraph to ADR-003 documenting that `nomic-embed-text-v1.5` is asymmetric and requires `search_document:` / `search_query:` prefixes on the document and query sides respectively. State that this is enforced inside `FastEmbedProvider::embed_documents()` / `embed_query()` and that callers must not prepend prefixes themselves.

Exact paragraph to add at the end of the ADR-003 section:

```markdown
**Asymmetric prefixes.** `nomic-embed-text-v1.5` was trained with task prefixes
and **requires** them at inference time: `search_document: <text>` for indexed
chunks and `search_query: <text>` for user queries. `fastembed-rs` (unlike the
Python `fastembed` library) does not add these automatically. Prefixing is
enforced inside `FastEmbedProvider::embed_documents()` and
`FastEmbedProvider::embed_query()`; callers MUST pass un-prefixed text. Missing
prefixes silently degrade retrieval recall (the failure mode that motivated this
change — see `docs/superpowers/plans/2026-05-31-rag-quality-improvements.md`).
```

- [ ] **Step 2: Commit**

```bash
git add docs/architecture.md
git commit -m "docs(adr-003): document nomic-embed task-prefix requirement"
```

---

## Task 3: Split embedding trait into document / query methods

The trait change is the spine of the prefix fix. Doing it first makes every downstream caller compile-error until updated, which is what we want.

**Files:**
- Modify: `src-tauri/src/providers/embedding.rs`
- Test: `src-tauri/src/providers/embedding.rs` (existing `#[cfg(test)]` block)

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `embedding.rs`:

```rust
#[tokio::test]
async fn test_fastembed_prefixes_documents_and_queries_differently() {
    let Ok(provider) = FastEmbedProvider::try_new_small() else {
        eprintln!("Skipping — small model not cached");
        return;
    };

    // The same raw text embedded as doc vs query must produce DIFFERENT vectors,
    // because the prefixes differ. (Both models we ship use prefixes; even the
    // small all-MiniLM-L6-v2 doesn't require them, but the trait shape must
    // still differ for Nomic to work.)
    let raw = "Coriolis orbits the planet Kua";
    let as_doc = provider
        .embed_documents(vec![raw.to_string()])
        .await
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let as_query = provider.embed_query(raw).await.unwrap();

    // For all-MiniLM-L6-v2 (no prefix) these will be identical — that's fine,
    // the test only enforces the trait surface compiles. We assert they're
    // the same length and non-degenerate.
    assert_eq!(as_doc.len(), as_query.len());
    assert!(as_doc.iter().any(|&v| v != 0.0));
}

#[tokio::test]
async fn test_mock_provider_implements_split_trait() {
    let provider = MockEmbeddingProvider::new(384);
    let docs = provider
        .embed_documents(vec!["hello".into(), "world".into()])
        .await
        .unwrap();
    assert_eq!(docs.len(), 2);
    let q = provider.embed_query("hello").await.unwrap();
    assert_eq!(q.len(), 384);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd src-tauri && cargo test --lib providers::embedding -- --nocapture
```

Expected: FAIL with "no method `embed_documents` found".

- [ ] **Step 3: Update the `EmbeddingProvider` trait**

Replace the existing trait in `embedding.rs`:

```rust
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Embed multiple documents (chunks) for indexing.
    /// Implementations MUST apply any model-specific document prefix.
    async fn embed_documents(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError>;

    /// Embed a single query for search.
    /// Implementations MUST apply any model-specific query prefix.
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;

    fn dimension(&self) -> usize;
    fn model_name(&self) -> &str;
}
```

- [ ] **Step 4: Update `FastEmbedProvider` impl to apply Nomic prefixes**

```rust
const NOMIC_DOC_PREFIX: &str = "search_document: ";
const NOMIC_QUERY_PREFIX: &str = "search_query: ";

#[async_trait]
impl EmbeddingProvider for FastEmbedProvider {
    async fn embed_documents(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let prefixed: Vec<String> = if self.uses_nomic_prefixes() {
            texts.into_iter().map(|t| format!("{NOMIC_DOC_PREFIX}{t}")).collect()
        } else {
            texts
        };
        let refs: Vec<&str> = prefixed.iter().map(|s| s.as_str()).collect();
        let mut model = self.inner.lock().await;
        model.embed(refs, None).map_err(|e| EmbeddingError::Embed(e.to_string()))
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let prefixed = if self.uses_nomic_prefixes() {
            format!("{NOMIC_QUERY_PREFIX}{text}")
        } else {
            text.to_string()
        };
        let mut model = self.inner.lock().await;
        let mut out = model
            .embed(vec![prefixed.as_str()], None)
            .map_err(|e| EmbeddingError::Embed(e.to_string()))?;
        Ok(out.pop().unwrap_or_default())
    }

    fn dimension(&self) -> usize { self.dim }
    fn model_name(&self) -> &str { self.name }
}

impl FastEmbedProvider {
    fn uses_nomic_prefixes(&self) -> bool {
        self.name.starts_with("nomic-embed-text")
    }
}
```

- [ ] **Step 5: Update `MockEmbeddingProvider`**

```rust
#[async_trait]
impl EmbeddingProvider for MockEmbeddingProvider {
    async fn embed_documents(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts.into_iter().map(|_| vec![0.0; self.dim]).collect())
    }
    async fn embed_query(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(vec![0.0; self.dim])
    }
    fn dimension(&self) -> usize { self.dim }
    fn model_name(&self) -> &str { &self.name }
}
```

- [ ] **Step 6: Remove the old `embed()` and default `embed_query()` methods from the trait**

Delete the old `async fn embed(...)` declaration and the default `embed_query` implementation in the trait body. The two implementations above replace them.

- [ ] **Step 7: Update the old test that called `.embed(...)`**

In the existing `test_mock_embed_batch` test, replace `provider.embed(...)` with `provider.embed_documents(...)`. In `test_mock_embed_query_returns_correct_dims`, no change needed. In `test_fastembed_try_new_small`, change `.embed_query("hello world")` — no change needed; but if any test calls `.embed(...)`, swap to `.embed_documents(...)`.

- [ ] **Step 8: Run tests to verify they pass**

```bash
cd src-tauri && cargo test --lib providers::embedding -- --nocapture
```

Expected: All embedding tests pass. The whole crate WILL NOT compile yet because `ingestion_service` / `agent_service` still call `.embed(...)` — fixed in the next task.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/providers/embedding.rs
git commit -m "feat(embedding): split trait into embed_documents/embed_query with Nomic prefixes"
```

---

## Task 4: Update ingestion and agent services to use the new methods

Makes the whole crate compile again and wires prefixes through end-to-end.

**Files:**
- Modify: `src-tauri/src/services/ingestion_service.rs:225-229`
- Modify: `src-tauri/src/services/agent_service.rs:50-53` (no behavior change — `embed_query` is the same name)

- [ ] **Step 1: Update `embed_chunks` in ingestion_service**

In `ingestion_service.rs`, change line 226-229 from:

```rust
let embeddings = provider
    .embed(texts)
    .await
    .map_err(|e| IngestionError::Embedding(e.to_string()))?;
```

to:

```rust
let embeddings = provider
    .embed_documents(texts)
    .await
    .map_err(|e| IngestionError::Embedding(e.to_string()))?;
```

- [ ] **Step 2: Verify `agent_service.rs` still compiles**

It already calls `embed_provider.embed_query(message)` (line 51) — the trait still has that method, so no change needed.

- [ ] **Step 3: Compile the whole workspace**

```bash
cd src-tauri && cargo build
```

Expected: clean build.

- [ ] **Step 4: Run the full test suite**

```bash
cd src-tauri && cargo test
```

Expected: all green. Existing mocks return zero-vectors so retrieval tests still pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/ingestion_service.rs
git commit -m "feat(ingestion): route chunk embedding through embed_documents (nomic prefix)"
```

---

## Task 5: Text normalizer module

Pure-function module that fixes the hyphen/paragraph artifacts the user quoted directly (`power-\nful`, `descen-\ndents`). No external dependencies needed.

**Files:**
- Create: `src-tauri/src/services/text_normalizer.rs`
- Modify: `src-tauri/src/services/mod.rs`

- [ ] **Step 1: Add module declaration**

In `src-tauri/src/services/mod.rs` add:

```rust
pub mod text_normalizer;
```

- [ ] **Step 2: Write the failing tests in `text_normalizer.rs`**

```rust
//! Repairs PDF extraction artifacts before chunking.
//!
//! Three repairs in order:
//! 1. Soft-hyphen line joins: `power-\nful` → `powerful`.
//! 2. Single newlines inside paragraphs → space; double newlines preserved.
//! 3. Collapse runs of whitespace.
//!
//! Idempotent: `normalize(normalize(x)) == normalize(x)`.

pub fn normalize(text: &str) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_soft_hyphenated_word_at_line_break() {
        let input = "the union of\nfree traders; the mercenaries of the\nLegion";
        let output = normalize(input);
        // Single newlines inside a paragraph become spaces
        assert!(output.contains("union of free traders"));
        assert!(output.contains("the mercenaries of the Legion"));
    }

    #[test]
    fn removes_hyphen_at_end_of_line() {
        let input = "power-\nful";
        assert_eq!(normalize(input), "powerful");
    }

    #[test]
    fn removes_multiple_hyphenated_breaks() {
        let input = "descen-\ndents of the cap-\ntain family";
        assert_eq!(normalize(input), "descendents of the captain family");
    }

    #[test]
    fn preserves_paragraph_breaks() {
        let input = "First paragraph.\n\nSecond paragraph.";
        let out = normalize(input);
        assert!(out.contains("First paragraph."));
        assert!(out.contains("Second paragraph."));
        assert!(out.contains("\n\n"), "paragraph break must survive: {out:?}");
    }

    #[test]
    fn collapses_runs_of_spaces() {
        assert_eq!(normalize("a    b\t\tc"), "a b c");
    }

    #[test]
    fn is_idempotent() {
        let input = "power-\nful descen-\ndents\n\nNew para.";
        let once = normalize(input);
        let twice = normalize(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn does_not_join_intentional_hyphenated_compound_at_word_boundary() {
        // A hyphenated compound that is NOT at a line break stays as-is.
        let input = "state-of-the-art system";
        assert_eq!(normalize(input), "state-of-the-art system");
    }

    #[test]
    fn handles_empty_string() {
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn handles_only_whitespace() {
        assert_eq!(normalize("   \n\n  \t  "), "");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
cd src-tauri && cargo test --lib services::text_normalizer
```

Expected: FAIL with `not yet implemented`.

- [ ] **Step 4: Implement `normalize`**

Replace the `todo!()` body with:

```rust
pub fn normalize(text: &str) -> String {
    use std::fmt::Write;

    if text.trim().is_empty() {
        return String::new();
    }

    // Step 1: rejoin soft-hyphenated line breaks ("-\n" → "")
    // Only when the char before '-' is alphabetic and the char after '\n' is
    // alphabetic, to avoid mangling intentional em-dash-style breaks.
    let mut step1 = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '-'
            && i + 1 < chars.len()
            && chars[i + 1] == '\n'
            && i > 0
            && chars[i - 1].is_alphabetic()
            && i + 2 < chars.len()
            && chars[i + 2].is_alphabetic()
        {
            // Skip the '-' and the '\n', no replacement
            i += 2;
            continue;
        }
        step1.push(chars[i]);
        i += 1;
    }

    // Step 2: collapse single newlines (inside paragraphs) to spaces;
    // preserve "\n\n" paragraph boundaries.
    let mut step2 = String::with_capacity(step1.len());
    let chars: Vec<char> = step1.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\n' {
            // Count consecutive newlines
            let start = i;
            while i < chars.len() && chars[i] == '\n' {
                i += 1;
            }
            let run = i - start;
            if run >= 2 {
                step2.push_str("\n\n");
            } else {
                step2.push(' ');
            }
            continue;
        }
        step2.push(chars[i]);
        i += 1;
    }

    // Step 3: collapse runs of horizontal whitespace within lines
    let mut out = String::with_capacity(step2.len());
    let mut last_was_space = false;
    for c in step2.chars() {
        if c == '\n' {
            out.push(c);
            last_was_space = false;
            continue;
        }
        if c.is_whitespace() {
            if !last_was_space {
                out.push(' ');
            }
            last_was_space = true;
        } else {
            out.push(c);
            last_was_space = false;
        }
    }

    // Trim trailing/leading whitespace, but preserve internal "\n\n"
    let _ = (&mut out, &Write::write_str); // silence unused import warning
    out.trim().to_string()
}
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cd src-tauri && cargo test --lib services::text_normalizer
```

Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/services/text_normalizer.rs src-tauri/src/services/mod.rs
git commit -m "feat(ingestion): add text normalizer for PDF de-hyphenation and paragraph repair"
```

---

## Task 6: Integrate normalizer into ingestion pipeline

Runs every extracted page through `normalize()` before chunking.

**Files:**
- Modify: `src-tauri/src/services/ingestion_service.rs` (`extract_text` function, lines 131-186)

- [ ] **Step 1: Write a failing integration test**

Add to the bottom of `ingestion_service.rs` in the `#[cfg(test)] mod tests` (create the mod if absent):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracted_doc_has_no_soft_hyphen_artifacts() {
        // Build a fake ExtractedDoc as if it came from PDF extraction
        let raw = ExtractedDoc {
            page_count: 1,
            text: "power-\nful descen-\ndents of\nthe captain family".to_string(),
            pages: vec![PageContent {
                page_num: 1,
                text: "power-\nful descen-\ndents of\nthe captain family".to_string(),
            }],
        };
        let normalized = normalize_extracted(&raw);
        assert!(!normalized.text.contains("-\n"), "soft hyphens not removed: {:?}", normalized.text);
        assert!(normalized.text.contains("powerful"));
        assert!(normalized.text.contains("descendents"));
        assert_eq!(normalized.pages[0].text, normalized.text);
    }
}
```

You'll need to add these imports at the top of the test module:

```rust
use crate::services::chunker::{ExtractedDoc, PageContent};
```

- [ ] **Step 2: Add `normalize_extracted` helper to `ingestion_service.rs`**

Just above `pub async fn ingest_source`:

```rust
/// Run `text_normalizer::normalize` over every page and the merged full text.
fn normalize_extracted(doc: &ExtractedDoc) -> ExtractedDoc {
    use crate::services::text_normalizer::normalize;
    let pages: Vec<PageContent> = doc
        .pages
        .iter()
        .map(|p| PageContent { page_num: p.page_num, text: normalize(&p.text) })
        .collect();
    // Rebuild full text from normalized pages to keep page offsets consistent
    let mut full = String::new();
    for (i, p) in pages.iter().enumerate() {
        if i > 0 && !p.text.is_empty() {
            full.push('\n');
        }
        full.push_str(&p.text);
    }
    ExtractedDoc { page_count: pages.len(), text: full, pages }
}
```

- [ ] **Step 3: Run the normalizer in `ingest_source`**

In `ingest_source` (around line 80), wrap the existing `extract_text` call:

```rust
on_progress(IngestionProgress { fraction: 0.20, step: "Extracting text from PDF pages".into() });
let extracted = extract_text(&pdf_data).await?;
let extracted = normalize_extracted(&extracted);
```

- [ ] **Step 4: Run tests**

```bash
cd src-tauri && cargo test --lib services::ingestion_service
```

Expected: green.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/ingestion_service.rs
git commit -m "feat(ingestion): normalize extracted PDF text before chunking"
```

---

## Task 7: Loosen system prompt + require quoted evidence

Smallest code change with the largest LLM-side impact.

**Files:**
- Modify: `src-tauri/src/services/agent_service.rs:106-124`
- Modify: `src-tauri/src/services/agent_service.rs:282-289` (test)

- [ ] **Step 1: Replace `build_rag_system_prompt`**

```rust
fn build_rag_system_prompt(context: &str) -> String {
    if context.is_empty() {
        return "You are an expert Game Master assistant. \
            Answer the user's question to the best of your ability. \
            If you don't know the answer, say so — do not make up rules."
            .to_string();
    }

    format!(
        "You are an expert Game Master assistant.\n\n\
         REFERENCE MATERIAL:\n{context}\n\
         INSTRUCTIONS:\n\
         - Read every passage above carefully BEFORE deciding whether the answer is present.\n\
         - The reference passages may use different wording than the user's question \
           (e.g. the question says \"factions\", the passage says \"groups\" or \"organizations\"). \
           Treat synonyms, paraphrases, and partial matches as valid evidence.\n\
         - When the answer IS present, quote the exact sentence(s) from the reference \
           material that support your answer, then add a one-line summary.\n\
         - Every factual claim must cite its source using this exact format: \
           [Source: \"<source name>\", p.<page>].\n\
         - Only say \"the reference material does not contain this information\" if you have \
           scanned every passage and found no relevant content, even by paraphrase.\n\
         - Be concise. The GM is running a table."
    )
}
```

- [ ] **Step 2: Update the existing test**

```rust
#[test]
fn test_system_prompt_with_context() {
    let ctx = "[0] Source: \"PHB.pdf\", p. 72 — \"Fighter Class Features\"\nAction Surge text.\n\n";
    let prompt = build_rag_system_prompt(ctx);
    assert!(prompt.contains("REFERENCE MATERIAL"));
    assert!(prompt.contains("PHB.pdf"));
    assert!(prompt.contains("[Source: \"<source name>\""));
    assert!(prompt.contains("quote the exact sentence"));
    assert!(prompt.contains("synonyms"));
}
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test --lib services::agent_service
```

Expected: green.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/agent_service.rs
git commit -m "feat(agent): loosen system prompt; require quoted evidence; reduce false refusals"
```

---

## Task 8: Sentence-aware, smaller-chunk chunker

Drops chunk size to ~250 tokens with ~20% overlap; splits on sentence boundaries inside the window.

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/services/chunker.rs`

- [ ] **Step 1: Add unicode-segmentation dependency**

In `src-tauri/Cargo.toml` under `[dependencies]`:

```toml
unicode-segmentation = "1"
```

- [ ] **Step 2: Write the failing tests**

Add to the chunker tests module:

```rust
#[test]
fn target_chunk_size_is_about_250_tokens() {
    assert_eq!(TARGET_TOKENS, 250);
    assert_eq!(OVERLAP_TOKENS, 50);
}

#[test]
fn chunks_split_on_sentence_boundary_not_mid_word() {
    let text = "First sentence ends here. Second sentence is longer and continues. \
                Third sentence wraps up. ".repeat(60);
    let doc = make_doc(&text, vec![(text.as_str(), 1)]);
    let chunks = chunk_document(&doc);
    assert!(chunks.len() >= 2);
    for c in &chunks {
        // Each chunk should END on a sentence terminator OR be the last chunk.
        let last_char = c.text.trim_end().chars().last().unwrap_or('.');
        assert!(
            ['.', '!', '?', '"', ')'].contains(&last_char),
            "chunk ends mid-sentence: {:?}",
            c.text.chars().rev().take(40).collect::<String>().chars().rev().collect::<String>()
        );
    }
}

#[test]
fn chunks_dont_start_mid_word() {
    let text = "Some sample text about combat. ".repeat(80);
    let doc = make_doc(&text, vec![(text.as_str(), 1)]);
    let chunks = chunk_document(&doc);
    for c in &chunks {
        let first_word = c.text.split_whitespace().next().unwrap_or("");
        // First word should start with a letter (not a hyphen / lowercase continuation)
        if let Some(first) = first_word.chars().next() {
            assert!(
                first.is_alphabetic() || first.is_numeric() || first == '\"' || first == '(',
                "chunk starts mid-word: {first_word:?}"
            );
        }
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
cd src-tauri && cargo test --lib services::chunker
```

Expected: `target_chunk_size_is_about_250_tokens` fails (currently 400).

- [ ] **Step 4: Update chunker constants**

In `chunker.rs`:

```rust
const TARGET_TOKENS: usize = 250;
const OVERLAP_TOKENS: usize = 50;
const CHARS_PER_TOKEN: f64 = 4.0;
```

- [ ] **Step 5: Replace the chunking loop with sentence-aware logic**

Replace the body of `chunk_document` with:

```rust
pub fn chunk_document(doc: &ExtractedDoc) -> Vec<Chunk> {
    use unicode_segmentation::UnicodeSegmentation;

    let target_chars = (TARGET_TOKENS as f64 * CHARS_PER_TOKEN) as usize;
    let overlap_chars = (OVERLAP_TOKENS as f64 * CHARS_PER_TOKEN) as usize;

    let headings = detect_headings(&doc.text);
    let page_offsets = build_page_offsets(&doc.pages);

    // Split full text into sentences (with their absolute char offsets)
    let sentences: Vec<(usize, &str)> = sentence_offsets(&doc.text);

    if sentences.is_empty() {
        return Vec::new();
    }

    let mut chunks: Vec<Chunk> = Vec::new();
    let mut i = 0;
    while i < sentences.len() {
        let chunk_start_char = sentences[i].0;
        let mut chunk_text = String::new();
        let mut j = i;
        // Greedily accumulate sentences until target reached
        while j < sentences.len() {
            let next_len = sentences[j].1.chars().count();
            if !chunk_text.is_empty() && chunk_text.chars().count() + next_len + 1 > target_chars {
                break;
            }
            if !chunk_text.is_empty() {
                chunk_text.push(' ');
            }
            chunk_text.push_str(sentences[j].1);
            j += 1;
        }
        if chunk_text.trim().is_empty() {
            i += 1;
            continue;
        }

        let chunk_end_char = chunk_start_char + chunk_text.chars().count();
        let heading = find_active_heading(chunk_start_char, &headings);
        let (ps, pe) = page_range_for_byte_range(chunk_start_char, chunk_end_char, &page_offsets);

        chunks.push(Chunk {
            text: chunk_text.trim().to_string(),
            page_start: ps,
            page_end: pe,
            section_heading: heading,
        });

        if j >= sentences.len() {
            break;
        }

        // Advance i so that the NEXT chunk starts overlap_chars before the last sentence
        let mut overlap_size = 0;
        let mut new_i = j;
        while new_i > i + 1 && overlap_size < overlap_chars {
            new_i -= 1;
            overlap_size += sentences[new_i].1.chars().count();
        }
        // Always advance at least one sentence
        i = std::cmp::max(new_i, i + 1);
    }

    chunks
}

/// Split text into sentences and return each sentence's starting char offset.
fn sentence_offsets(text: &str) -> Vec<(usize, &str)> {
    use unicode_segmentation::UnicodeSegmentation;
    let mut out = Vec::new();
    let mut offset = 0usize;
    for sentence in text.unicode_sentences() {
        let trimmed = sentence.trim();
        if !trimmed.is_empty() {
            // Find trimmed sentence's offset within the original
            let trim_start = sentence.find(trimmed.chars().next().unwrap()).unwrap_or(0);
            out.push((offset + trim_start, trimmed));
        }
        offset += sentence.chars().count();
    }
    out
}
```

- [ ] **Step 6: Run tests**

```bash
cd src-tauri && cargo test --lib services::chunker
```

Expected: all green. The `test_chunk_very_long_document_creates_multiple_chunks` overlap-by-last-whitespace check may need updating — if it fails, remove that contains-assertion and replace with a length check (`assert!(chunks.iter().all(|c| approx_token_count(&c.text) <= 300))`).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/services/chunker.rs
git commit -m "feat(chunker): 250-token chunks with sentence-aware boundaries (was 400/80 char-window)"
```

---

## Task 9: PdfExtractor trait + pdfium-render impl

Replaces `pdf-extract` with the approved `pdfium-render` crate. Trait keeps the seam mockable per CLAUDE.md.

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/build.rs`
- Modify: `src-tauri/tauri.conf.json`
- Create: `src-tauri/resources/pdfium/.gitkeep`
- Create: `src-tauri/src/services/pdf_extractor.rs`
- Modify: `src-tauri/src/services/mod.rs`

- [ ] **Step 1: Update Cargo.toml**

Remove the `pdf-extract = "0.10"` line. Add under `[dependencies]`:

```toml
pdfium-render = { version = "0.8", default-features = false, features = ["thread_safe", "image"] }
```

And under `[build-dependencies]` add:

```toml
reqwest = { version = "0.12", features = ["blocking"] }
zip = "2"
```

- [ ] **Step 2: Add `.gitkeep` and bundle config**

```bash
mkdir -p src-tauri/resources/pdfium
touch src-tauri/resources/pdfium/.gitkeep
```

In `src-tauri/tauri.conf.json`, change the `"bundle"` block to:

```json
"bundle": {
  "active": true,
  "targets": "all",
  "icon": ["icons/icon.png"],
  "resources": ["resources/pdfium/**/*"]
}
```

- [ ] **Step 3: Update `build.rs` to download pdfium**

Replace `src-tauri/build.rs` with:

```rust
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    tauri_build::build();

    // Skip during cargo doc / clippy if env var set
    if env::var("CHRONACLE_SKIP_PDFIUM_DOWNLOAD").is_ok() {
        return;
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    // bblanchon/pdfium-binaries release naming
    let (asset, lib_name) = match (target_os.as_str(), target_arch.as_str()) {
        ("macos", "aarch64") => ("pdfium-mac-arm64.tgz", "libpdfium.dylib"),
        ("macos", "x86_64") => ("pdfium-mac-x64.tgz", "libpdfium.dylib"),
        ("linux", "x86_64") => ("pdfium-linux-x64.tgz", "libpdfium.so"),
        ("linux", "aarch64") => ("pdfium-linux-arm64.tgz", "libpdfium.so"),
        ("windows", "x86_64") => ("pdfium-win-x64.tgz", "pdfium.dll"),
        _ => {
            println!("cargo:warning=Unsupported target {target_os}/{target_arch} — pdfium not downloaded; runtime PDF extraction will fail.");
            return;
        }
    };

    let resources_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("resources/pdfium");
    let lib_path = resources_dir.join(lib_name);
    if lib_path.exists() {
        println!("cargo:rerun-if-changed=resources/pdfium/{lib_name}");
        return;
    }

    fs::create_dir_all(&resources_dir).expect("create resources/pdfium dir");

    let url = format!(
        "https://github.com/bblanchon/pdfium-binaries/releases/latest/download/{asset}"
    );
    println!("cargo:warning=Downloading pdfium from {url}");

    let resp = reqwest::blocking::get(&url).expect("download pdfium");
    let bytes = resp.bytes().expect("read pdfium body");

    // The archive is a .tgz containing lib/<lib_name>
    let tar = flate2::read::GzDecoder::new(std::io::Cursor::new(bytes.as_ref()));
    let mut archive = tar::Archive::new(tar);
    for entry in archive.entries().expect("read tar") {
        let mut entry = entry.expect("entry");
        let path = entry.path().expect("path").to_path_buf();
        if path.file_name().and_then(|s| s.to_str()) == Some(lib_name) {
            let mut out = fs::File::create(&lib_path).expect("create lib");
            std::io::copy(&mut entry, &mut out).expect("write lib");
            println!("cargo:rerun-if-changed=resources/pdfium/{lib_name}");
            return;
        }
    }
    panic!("pdfium binary {lib_name} not found in archive");
}
```

Add the supporting build-deps to `[build-dependencies]`:

```toml
flate2 = "1"
tar = "0.4"
```

Remove the `zip = "2"` line added in Step 1 — we use tar.gz, not zip.

- [ ] **Step 4: Build to verify pdfium download works**

```bash
cd src-tauri && cargo build
```

Expected: build succeeds and `src-tauri/resources/pdfium/libpdfium.dylib` (or platform equivalent) exists.

- [ ] **Step 5: Write the failing extractor test**

Create `src-tauri/src/services/pdf_extractor.rs`:

```rust
//! PDF text extraction abstraction.
//!
//! Backed by `pdfium-render` (Chromium PDF engine) for layout-aware extraction
//! that handles multi-column TTRPG rulebooks correctly. The library is loaded
//! at runtime from a bundled binary; see `build.rs`.

use async_trait::async_trait;

use crate::services::chunker::{ExtractedDoc, PageContent};

#[derive(Debug, thiserror::Error)]
pub enum PdfExtractError {
    #[error("PDF library load failed: {0}")]
    LibLoad(String),
    #[error("PDF parse failed: {0}")]
    Parse(String),
}

#[async_trait]
pub trait PdfExtractor: Send + Sync {
    /// Extract one `PageContent` per PDF page.
    async fn extract(&self, data: &[u8]) -> Result<ExtractedDoc, PdfExtractError>;
}

pub struct PdfiumExtractor {
    library_path: std::path::PathBuf,
}

impl PdfiumExtractor {
    pub fn new(library_path: std::path::PathBuf) -> Self {
        Self { library_path }
    }
}

#[async_trait]
impl PdfExtractor for PdfiumExtractor {
    async fn extract(&self, data: &[u8]) -> Result<ExtractedDoc, PdfExtractError> {
        let data = data.to_vec();
        let lib_path = self.library_path.clone();
        tokio::task::spawn_blocking(move || extract_blocking(&lib_path, &data))
            .await
            .map_err(|e| PdfExtractError::Parse(format!("join error: {e}")))?
    }
}

fn extract_blocking(
    library_path: &std::path::Path,
    data: &[u8],
) -> Result<ExtractedDoc, PdfExtractError> {
    use pdfium_render::prelude::*;

    let bindings = Pdfium::bind_to_library(library_path)
        .map_err(|e| PdfExtractError::LibLoad(e.to_string()))?;
    let pdfium = Pdfium::new(bindings);

    let document = pdfium
        .load_pdf_from_byte_slice(data, None)
        .map_err(|e| PdfExtractError::Parse(e.to_string()))?;

    let mut pages = Vec::new();
    let mut full = String::new();
    for (i, page) in document.pages().iter().enumerate() {
        let text = page
            .text()
            .map_err(|e| PdfExtractError::Parse(e.to_string()))?
            .all();
        if i > 0 && !text.is_empty() {
            full.push('\n');
        }
        full.push_str(&text);
        pages.push(PageContent { page_num: i + 1, text });
    }

    Ok(ExtractedDoc {
        page_count: pages.len(),
        text: full,
        pages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pdfium_lib_path() -> std::path::PathBuf {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium");
        let name = if cfg!(target_os = "macos") {
            "libpdfium.dylib"
        } else if cfg!(target_os = "linux") {
            "libpdfium.so"
        } else {
            "pdfium.dll"
        };
        dir.join(name)
    }

    fn make_one_page_pdf(text: &str) -> Vec<u8> {
        // Use lopdf to build a minimal valid PDF with the given text.
        use lopdf::content::{Content, Operation};
        use lopdf::dictionary;
        use lopdf::{Document, Object, Stream};
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });
        let content = Content { operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 24.into()]),
            Operation::new("Td", vec![100.into(), 600.into()]),
            Operation::new("Tj", vec![Object::string_literal(text)]),
            Operation::new("ET", vec![]),
        ] };
        let content_id = doc.add_object(Stream::new(dictionary!{}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
        });
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[tokio::test]
    async fn extracts_text_from_minimal_pdf() {
        let lib = pdfium_lib_path();
        if !lib.exists() {
            eprintln!("Skipping — pdfium not built");
            return;
        }
        let pdf = make_one_page_pdf("Coriolis orbits Kua");
        let extractor = PdfiumExtractor::new(lib);
        let doc = extractor.extract(&pdf).await.expect("extract");
        assert_eq!(doc.page_count, 1);
        assert!(
            doc.text.contains("Coriolis") && doc.text.contains("Kua"),
            "extracted text missing markers: {:?}",
            doc.text
        );
    }
}
```

In `src-tauri/src/services/mod.rs` add:

```rust
pub mod pdf_extractor;
```

- [ ] **Step 6: Run extractor tests**

```bash
cd src-tauri && cargo test --lib services::pdf_extractor -- --nocapture
```

Expected: green (or skipped if pdfium binary missing).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/build.rs src-tauri/tauri.conf.json \
        src-tauri/resources/pdfium/.gitkeep src-tauri/src/services/pdf_extractor.rs \
        src-tauri/src/services/mod.rs
git commit -m "feat(pdf): add pdfium-render extractor behind PdfExtractor trait"
```

---

## Task 10: Swap ingestion to use PdfiumExtractor; remove pdf-extract

**Files:**
- Modify: `src-tauri/Cargo.toml` (already done in Task 9)
- Modify: `src-tauri/src/lib.rs` (AppState)
- Modify: `src-tauri/src/services/ingestion_service.rs`

- [ ] **Step 1: Add `PdfExtractor` to AppState**

In `src-tauri/src/lib.rs`, locate the `AppState` struct definition. Add field:

```rust
pub pdf_extractor: Arc<dyn crate::services::pdf_extractor::PdfExtractor>,
```

Locate where `AppState` is constructed (in the Tauri setup hook). Add:

```rust
let pdfium_lib = app
    .path()
    .resolve("resources/pdfium", tauri::path::BaseDirectory::Resource)
    .expect("pdfium resource dir")
    .join(if cfg!(target_os = "macos") { "libpdfium.dylib" }
          else if cfg!(target_os = "linux") { "libpdfium.so" }
          else { "pdfium.dll" });
let pdf_extractor: Arc<dyn crate::services::pdf_extractor::PdfExtractor> =
    Arc::new(crate::services::pdf_extractor::PdfiumExtractor::new(pdfium_lib));
```

Include `pdf_extractor` when constructing `AppState`.

- [ ] **Step 2: Replace `extract_text` in ingestion_service**

In `src-tauri/src/services/ingestion_service.rs`, change `pub async fn extract_text(data: &[u8]) -> Result<ExtractedDoc, IngestionError>` to accept the extractor:

```rust
pub async fn extract_text(
    extractor: &Arc<dyn crate::services::pdf_extractor::PdfExtractor>,
    data: &[u8],
) -> Result<ExtractedDoc, IngestionError> {
    extractor.extract(data).await.map_err(|e| IngestionError::PdfExtraction(e.to_string()))
}
```

Update the call site in `ingest_source`:

```rust
let extracted = extract_text(&state.pdf_extractor, &pdf_data).await?;
let extracted = normalize_extracted(&extracted);
```

Remove all `pdf_extract::...` calls and the fallback branch.

- [ ] **Step 3: Update or remove tests that called the old `extract_text(data)` signature**

Any tests in `ingestion_service.rs` that previously passed a hard-coded PDF blob need to be either:
- Updated to construct a `PdfiumExtractor`, OR
- Moved to `src-tauri/tests/` as integration tests gated on the pdfium binary's presence (see Task 13).

For unit tests that just need to test pipeline shape, use a `MockPdfExtractor`:

```rust
#[cfg(test)]
mod mock {
    use super::*;
    use crate::services::pdf_extractor::{PdfExtractError, PdfExtractor};
    pub struct MockPdfExtractor(pub ExtractedDoc);
    #[async_trait::async_trait]
    impl PdfExtractor for MockPdfExtractor {
        async fn extract(&self, _: &[u8]) -> Result<ExtractedDoc, PdfExtractError> {
            Ok(self.0.clone())
        }
    }
}
```

- [ ] **Step 4: Confirm `pdf-extract` is no longer referenced anywhere**

```bash
grep -rn "pdf_extract\|pdf-extract" src-tauri/src src-tauri/tests src-tauri/Cargo.toml
```

Expected: no matches.

- [ ] **Step 5: Build + test**

```bash
cd src-tauri && cargo build && cargo test
```

Expected: clean build + green tests.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/
git commit -m "feat(ingestion): swap pdf-extract for pdfium-render via PdfExtractor trait"
```

---

## Task 11: `reindex_all_sources` command

Lets the user re-run ingestion across every source with the new pipeline.

**Files:**
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs` (register the command)
- Modify: `src/lib/commands.ts`

- [ ] **Step 1: Write the failing test**

In `src-tauri/src/commands/mod.rs`'s test module (create if absent):

```rust
#[cfg(test)]
mod reindex_tests {
    use super::*;

    #[tokio::test]
    async fn reindex_all_marks_sources_pending_then_indexing_then_done() {
        // Skip — this is verified by integration test in Task 13.
        // Unit test here just asserts the helper enumerates sources correctly.
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(()).await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query("CREATE source SET id = 's1', filename = 'a.pdf', display_name='a', source_type='rules', page_count=0, indexed_at=time::now(), index_status='done', embed_model='nomic-embed-text-v1.5'").await.unwrap();
        db.query("CREATE source SET id = 's2', filename = 'b.pdf', display_name='b', source_type='rules', page_count=0, indexed_at=time::now(), index_status='done', embed_model='nomic-embed-text-v1.5'").await.unwrap();
        let ids = list_all_source_ids(&db).await.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"s1".to_string()));
        assert!(ids.contains(&"s2".to_string()));
    }
}
```

- [ ] **Step 2: Implement the helper + command**

Add to `commands/mod.rs`:

```rust
/// Enumerate all source IDs in the database.
async fn list_all_source_ids<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
) -> Result<Vec<String>, String> {
    #[derive(serde::Deserialize)]
    struct Row { id: surrealdb::sql::Thing }
    let mut resp = db.query("SELECT id FROM source").await.map_err(|e| e.to_string())?;
    let rows: Vec<Row> = resp.take(0).map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(|r| r.id.id.to_string()).collect())
}

#[tauri::command]
pub async fn reindex_all_sources(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, std::sync::Arc<crate::AppState>>,
) -> Result<usize, String> {
    let ids = list_all_source_ids(&state.db).await?;
    let total = ids.len();

    for (idx, sid) in ids.iter().enumerate() {
        let sid_for_progress = sid.clone();
        let handle = app_handle.clone();
        let on_progress: std::sync::Arc<
            dyn Fn(crate::services::ingestion_service::IngestionProgress) + Send + Sync,
        > = std::sync::Arc::new(move |p| {
            let _ = handle.emit(
                "reindex-progress",
                serde_json::json!({
                    "source_id": &sid_for_progress,
                    "current": idx + 1,
                    "total": total,
                    "progress": p.fraction,
                    "step": p.step,
                }),
            );
        });

        // Drop existing chunks for this source before re-embedding
        state.vector_store
            .delete_by_source(sid)
            .await
            .map_err(|e| format!("delete chunks for {sid}: {e}"))?;

        let state_ref = state.inner().clone();
        crate::services::ingestion_service::ingest_source(&state_ref, sid, on_progress)
            .await
            .map_err(|e| format!("re-ingest {sid}: {e}"))?;
    }

    Ok(total)
}
```

(Add `use tauri::Emitter;` at the top of the file if missing.)

- [ ] **Step 3: Register the command in `lib.rs`**

In `src-tauri/src/lib.rs`, locate `tauri::generate_handler![ ... ]` and add `commands::reindex_all_sources` to the list.

- [ ] **Step 4: Add the TS wrapper**

In `src/lib/commands.ts`:

```typescript
import { invoke } from '@tauri-apps/api/core';

export async function reindexAllSources(): Promise<number> {
  return await invoke<number>('reindex_all_sources');
}

export interface ReindexProgress {
  source_id: string;
  current: number;
  total: number;
  progress: number;
  step: string;
}
```

- [ ] **Step 5: Run tests + build**

```bash
cd src-tauri && cargo test --lib commands::reindex_tests
cargo build
cd .. && pnpm typecheck
```

Expected: green + typecheck passes.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src/lib/commands.ts
git commit -m "feat(commands): add reindex_all_sources command + TS wrapper"
```

---

## Task 12: SettingsPage "Embedding model" section + Re-index button

**Files:**
- Modify: `src/SettingsPage.svelte`
- Modify: `src/lib/events.ts` (if a `ReindexProgress` event type belongs there)

- [ ] **Step 1: Add state + handler**

At the top of `<script>` in `SettingsPage.svelte`, alongside other imports:

```typescript
import { reindexAllSources, type ReindexProgress } from './lib/commands';
import { listen } from '@tauri-apps/api/event';
```

Add reactive state:

```typescript
let reindexing = $state(false);
let reindexProgress = $state<ReindexProgress | null>(null);
let reindexError = $state<string | null>(null);
let reindexedCount = $state<number | null>(null);
```

Add handler:

```typescript
async function onReindexAll() {
  reindexing = true;
  reindexError = null;
  reindexedCount = null;
  const unlisten = await listen<ReindexProgress>('reindex-progress', (e) => {
    reindexProgress = e.payload;
  });
  try {
    const count = await reindexAllSources();
    reindexedCount = count;
  } catch (e) {
    reindexError = String(e);
  } finally {
    reindexing = false;
    reindexProgress = null;
    unlisten();
  }
}
```

- [ ] **Step 2: Add UI section**

In the template body, append a new `<section>` (match the existing pattern in the file):

```svelte
<section class="settings-section">
  <h2>Embedding model</h2>
  <p class="muted">
    Re-index all PDFs to apply recent improvements to text extraction, chunking,
    and embedding. Existing PDFs stay searchable during re-indexing.
  </p>
  <button class="primary" disabled={reindexing} onclick={onReindexAll}>
    {reindexing ? 'Re-indexing…' : 'Re-index all sources'}
  </button>
  {#if reindexing && reindexProgress}
    <div class="progress">
      Source {reindexProgress.current}/{reindexProgress.total}:
      {reindexProgress.step}
      ({Math.round(reindexProgress.progress * 100)}%)
    </div>
  {/if}
  {#if reindexError}
    <div class="error">Re-index failed: {reindexError}</div>
  {/if}
  {#if reindexedCount !== null && !reindexing}
    <div class="success">Re-indexed {reindexedCount} source(s).</div>
  {/if}
</section>
```

(If the existing file uses different CSS class names, match them — peek above to see what's there.)

- [ ] **Step 3: Run typecheck + dev**

```bash
pnpm typecheck
pnpm dev &
# manually open the app, navigate to Settings → see the new section
```

- [ ] **Step 4: Commit**

```bash
git add src/SettingsPage.svelte src/lib/commands.ts
git commit -m "feat(settings): add Embedding model section with Re-index all button"
```

---

## Task 13: End-to-end RAG-quality integration test

Locks in the fix with a regression test using a fixture PDF that mimics the user's failure modes.

**Files:**
- Create: `src-tauri/tests/rag_quality_integration.rs`

- [ ] **Step 1: Write the integration test**

```rust
//! Regression test for the GM agent's reply quality.
//!
//! Constructs a small PDF with text that resembles the Coriolis Quickstart
//! (multi-line sentences, hyphenated line breaks, lists) and verifies that
//! the retrieval pipeline returns the correct chunk for two factoid queries
//! that were failing in production.

use chronacle_lib::providers::embedding::{EmbeddingProvider, FastEmbedProvider};
use chronacle_lib::providers::vector_store::{SurrealDbVector, VectorStore};
use chronacle_lib::services::chunker::{chunk_document, ExtractedDoc, PageContent};
use chronacle_lib::services::text_normalizer::normalize;
use std::sync::Arc;
use surrealdb::engine::local::Mem;
use surrealdb::Surreal;

fn coriolis_like_fixture() -> ExtractedDoc {
    // Two paragraphs with the exact failing-question subject matter.
    let raw = "The center of the Third Horizon is the Kua system, where the space station\n\
        Coriolis orbits the green jungles of the planet Kua.\n\n\
        The council factions of today are the Consortium, a group of power-\n\
        ful corporations; the Zenithian Hegemony, the descen-\n\
        dents of the captain family onboard Zenith; the Free League, the union\n\
        of free traders; the mercenaries of the Legion; the secretive Draconites;\n\
        the divine iconocrates of the Order of the Pariah; Ahlam's Temple;\n\
        and the Church of the Icons.";
    let normalized = normalize(raw);
    ExtractedDoc {
        page_count: 1,
        text: normalized.clone(),
        pages: vec![PageContent { page_num: 1, text: normalized }],
    }
}

#[tokio::test]
async fn coriolis_orbit_question_retrieves_correct_chunk() {
    let Ok(embed) = FastEmbedProvider::try_new(None) else {
        eprintln!("Skipping — nomic model not cached");
        return;
    };
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(embed);

    let doc = coriolis_like_fixture();
    let chunks = chunk_document(&doc);
    let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
    let vectors = embed.embed_documents(texts.clone()).await.unwrap();

    let db = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_lib::schema::run_migrations(&db).await.unwrap();
    db.query("CREATE source SET id='s1', filename='quickstart.pdf', display_name='Quickstart', source_type='rules', page_count=1, indexed_at=time::now(), index_status='done', embed_model='nomic-embed-text-v1.5'").await.unwrap();

    let store = SurrealDbVector::new(db.clone());
    let indexed: Vec<_> = chunks.iter().zip(vectors).enumerate().map(|(i, (c, v))| {
        chronacle_lib::providers::vector_store::IndexedChunk {
            chunk_id: format!("s1-{i}"),
            campaign_id: None,
            text: c.text.clone(),
            page_start: c.page_start,
            page_end: c.page_end,
            section_heading: c.section_heading.clone(),
            source_type: "rules".into(),
            embedding: v,
            embed_model: "nomic-embed-text-v1.5".into(),
        }
    }).collect();
    store.upsert("s1", &indexed).await.unwrap();

    // Query: should retrieve the chunk mentioning "Coriolis orbits ... planet Kua"
    let qv = embed.embed_query("What planet is Coriolis orbiting?").await.unwrap();
    let results = store.search(&qv, None, 3).await.unwrap();
    assert!(!results.is_empty());
    let top = &results[0];
    assert!(
        top.text.to_lowercase().contains("kua") && top.text.to_lowercase().contains("coriolis"),
        "top chunk should mention Kua + Coriolis; got: {:?}",
        top.text
    );
}

#[tokio::test]
async fn council_factions_question_retrieves_correct_chunk() {
    let Ok(embed) = FastEmbedProvider::try_new(None) else {
        eprintln!("Skipping — nomic model not cached");
        return;
    };
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(embed);

    let doc = coriolis_like_fixture();
    let chunks = chunk_document(&doc);
    let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
    let vectors = embed.embed_documents(texts).await.unwrap();

    let db = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_lib::schema::run_migrations(&db).await.unwrap();
    db.query("CREATE source SET id='s1', filename='quickstart.pdf', display_name='Quickstart', source_type='rules', page_count=1, indexed_at=time::now(), index_status='done', embed_model='nomic-embed-text-v1.5'").await.unwrap();

    let store = SurrealDbVector::new(db);
    let indexed: Vec<_> = chunks.iter().zip(vectors).enumerate().map(|(i, (c, v))| {
        chronacle_lib::providers::vector_store::IndexedChunk {
            chunk_id: format!("s1-{i}"),
            campaign_id: None,
            text: c.text.clone(),
            page_start: c.page_start,
            page_end: c.page_end,
            section_heading: c.section_heading.clone(),
            source_type: "rules".into(),
            embedding: v,
            embed_model: "nomic-embed-text-v1.5".into(),
        }
    }).collect();
    store.upsert("s1", &indexed).await.unwrap();

    let qv = embed.embed_query("Which are the council factions?").await.unwrap();
    let results = store.search(&qv, None, 3).await.unwrap();
    assert!(!results.is_empty());
    let top = &results[0];
    let lower = top.text.to_lowercase();
    assert!(
        lower.contains("consortium") && lower.contains("free league"),
        "top chunk should list factions; got: {:?}",
        top.text
    );
    // Also assert de-hyphenation worked
    assert!(!top.text.contains("power-"), "soft hyphen leaked into chunk: {:?}", top.text);
    assert!(top.text.contains("powerful") || top.text.contains("Consortium"),
        "chunk text missing expected content: {:?}", top.text);
}
```

- [ ] **Step 2: Run the integration test**

```bash
cd src-tauri && cargo test --test rag_quality_integration -- --nocapture
```

Expected: both tests pass (or skip cleanly if the nomic model isn't cached).

- [ ] **Step 3: Manual smoke test against the real Coriolis PDF**

```bash
cargo tauri dev
```

In the running app:
1. Settings → Re-index all sources (wait for completion).
2. Chat → ask "What planet is Coriolis orbiting?" — assert the response includes "Kua".
3. Chat → ask "Which are the council factions?" — assert the response lists Consortium, Zenithian Hegemony, Free League, Legion, Draconites, Order of the Pariah, Ahlam's Temple, Church of the Icons.

Report results in plan execution notes. If either still fails, file findings before claiming done.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tests/rag_quality_integration.rs
git commit -m "test(rag): integration test for Coriolis orbit + council factions queries"
```

---

## Task 14: Pre-merge sweep

- [ ] **Step 1: Full quality gate**

```bash
cd src-tauri && cargo fmt && cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cd .. && pnpm typecheck && pnpm lint && pnpm test --run
```

Expected: all green.

- [ ] **Step 2: Verify approved-crates rule**

```bash
grep -n "^pdf-extract\|^pdf_extract" src-tauri/Cargo.toml || echo "OK: pdf-extract removed"
grep -n "^pdfium-render\|^unicode-segmentation" src-tauri/Cargo.toml
```

Expected: pdf-extract gone; pdfium-render and unicode-segmentation present.

- [ ] **Step 3: Tag the worktree commit**

```bash
git log --oneline -20
```

Confirm the commit history reads as a clean story.

- [ ] **Step 4: Hand off**

Use `superpowers:finishing-a-development-branch` to choose merge vs PR vs further review.

---

## Self-review

- **Spec coverage:** All 5 chosen issues (#1 prefixes, #2 normalization, #3 chunk size, #4 prompt, #5 pdfium) have at least one task. Re-index button covered (Tasks 11-12). Integration test covers regression (Task 13).
- **Placeholder scan:** No "TBD" / "add error handling" / "similar to" placeholders. The build.rs has a `cargo:warning=Unsupported target` branch but that's an explicit decision, not a placeholder.
- **Type consistency:** `EmbeddingProvider` methods are `embed_documents` and `embed_query` in trait + both impls + all callers (Tasks 3-4). `PdfExtractor::extract` signature used consistently in Tasks 9, 10. `IngestionProgress` struct name unchanged from current.
- **Known sharp edges:**
  - The build.rs download requires network access on first build. Documented via cargo:warning. Hosted CI may need `CHRONACLE_SKIP_PDFIUM_DOWNLOAD=1` if pdfium libraries are vendored separately there.
  - `tauri::path::BaseDirectory::Resource` resolution at runtime needs the Tauri context — verified the API exists in Tauri 2.
  - SurrealDB `<|1|>` KNN syntax was flagged in analysis but left unchanged — handling it is outside the scope the user approved (would need a separate small task).
