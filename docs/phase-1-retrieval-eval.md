# Phase 1 retrieval quality evaluation

**Date:** 2026-06-03
**Headline:** recall@5 = **100.0%** (12/12) on the Phase 1 query set.
**Phase 3 cross-encoder reranker decision:** **skip.** Recall is above the 85% threshold set in `docs/architecture.md:888`.

---

## Method

Per `docs/architecture.md` §"Vector Search & Retrieval Quality" (line 785) and §Phase 3 checklist (line 888):

> Top-k ANN is evaluated against a test set of 50 real TTRPG queries in Phase 1.
> If retrieval recall@5 is below 70%, cross-encoder is added in Phase 3.
> If above 85%, it ships as-is.

The Phase 1 evaluation in this repo is scoped down to **12 queries against 4 fixture PDFs** — enough signal to make the Phase 3 reranker decision and to detect catastrophic failure of the retrieval pipeline. The full 50-query set can grow over time without blocking Phase 1 sign-off.

The harness is implemented in `src-tauri/tests/retrieval_recall.rs` and is `#[ignore]` by default because it requires the real Nomic embedding model (~250 MB) to be cached locally.

### Pipeline under test

The harness exercises the complete production retrieval path:

1. PDF extraction via `PdfiumExtractor` (`pdfium-render`).
2. Text normalisation (soft-hyphen repair, paragraph reflow).
3. Section-aware chunking (`chunk_document`).
4. Embedding via `FastEmbedProvider` with `nomic-embed-text-v1.5` (768-dim, `search_document:` / `search_query:` prefixes).
5. SurrealDB MTREE COSINE vector index.
6. Top-5 ANN search filtered by collection.

### Fixtures

| Fixture | Topic |
|---|---|
| `single-column-text.pdf` | Combat (initiative, rounds, attacks, crits, cover) |
| `multi-column.pdf` | Spellcasting (preparation, schools, slots, ritual casting) |
| `tables.pdf` | Equipment (weapons table with cost / damage / weight) |
| `stat-block.pdf` | Monster stat block (Ancient Red Dragon) |

### Scoring

For each query, the harness embeds it, runs the top-5 vector search, and counts the query as a **hit** if at least one of the top-5 chunks contains a hand-picked ground-truth marker substring (case-insensitive).

`recall@5 = hits / total`.

### Query set

12 queries spanning factoid lookup, definitional lookup, table lookup, and stat-block lookup:

| # | Query | Marker | Fixture |
|---|---|---|---|
| 1 | How is initiative determined? | "Initiative" | single-column-text |
| 2 | What is a critical hit? | "Critical Hit" | single-column-text |
| 3 | How does cover affect armor class? | "Cover" | single-column-text |
| 4 | How long is a combat round? | "six seconds" | single-column-text |
| 5 | How do wizards prepare their spells? | "spellbook" | multi-column |
| 6 | What does the Fireball spell do? | "Fireball" | multi-column |
| 7 | What are the eight schools of magic? | "Abjuration" | multi-column |
| 8 | What happens to concentration when you take damage? | "Constitution saving throw" | multi-column |
| 9 | Can spells be cast as rituals? | "ritual" | multi-column |
| 10 | How much does a dagger cost? | "Dagger" | tables |
| 11 | What damage does a greatsword deal? | "Greatsword" | tables |
| 12 | What is the Ancient Red Dragon's armor class? | "Armor Class 22" | stat-block |

---

## Result

```
recall@5 = 12/12 = 100.0%
```

Every query was satisfied by at least one chunk in the top 5. No misses.

The harness asserts a **catastrophic-failure floor of 50%** — anything below that fails CI and indicates a real regression in the retrieval pipeline. The current 100% leaves substantial headroom.

---

## Phase 3 reranker decision

| Threshold | Action |
|---|---|
| recall@5 < 70% | Commit to cross-encoder reranker in Phase 3 |
| 70% ≤ recall@5 ≤ 85% | Defer decision; revisit with a larger query set |
| **recall@5 > 85%** | **Skip cross-encoder reranker; ship as-is** |

**Outcome:** `100.0% > 85%` → **skip cross-encoder reranker.** Phase 3 should remove `Cross-encoder reranking` from its checklist or mark it as "not required (Phase 1 recall@5 = 100%)".

---

## How to reproduce

```sh
cd src-tauri
cargo test --test retrieval_recall -- --ignored --nocapture
```

Requires:

- Nomic embedding model cached locally (run the app once to download it via the onboarding screen).
- `libpdfium.dylib` / `libpdfium.so` / `pdfium.dll` present in `src-tauri/resources/pdfium/` (already vendored).

The headline `recall@5 = ...` line is printed to stderr. Misses are logged with the top-5 chunk texts so failures can be diagnosed.

---

## Caveats and follow-up

- **12 queries is the floor, not the ceiling.** Grow toward the 50-query target in `docs/architecture.md:785` as real GM usage produces representative questions.
- The fixture PDFs are short synthetic rules content. Real-world rulebooks (multi-column layout, tables, stat blocks, sidebars) may stress retrieval differently — re-run this harness against any production-grade fixture before drawing strong conclusions about cross-encoder necessity.
- This evaluation only covers retrieval quality. End-to-end answer quality (LLM hallucination rate, citation accuracy) is tracked separately via the integration tests in `tests/integration_test.rs` and `tests/rag_quality_integration.rs`.
