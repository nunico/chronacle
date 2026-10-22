# Reliability and UX user stories

**Status:** Proposed; all stories open. **Baseline:** `f194470`.
[Delivery order](../../roadmap.md) · [Evidence and finding IDs](../../audits/2026-10-22-project-audit.md).

## Use one story at a time

1. Confirm the cited behavior still exists.
2. Read **Depends**; finish prerequisites first.
3. Turn the Gherkin into executable acceptance and add the listed boundary tests.
4. Implement the smallest change that passes. Update docs and locale copy where behavior changes.
5. Record verification; update story and roadmap status after acceptance passes.

**Ready** means the outcome is defined; it does not waive implementation design, schema review, or ADR requirements. **Needs spec** means resolve the named product contract before implementation. Paths below are relative to the repository root; they are starting points, not an exhaustive change list.

## Shared verification

- **Rust:** use the affected crate's unit/in-memory integration tests with mocked providers. Commands: `cargo test -p chronacle-ingestion`, `cargo test -p chronacle-providers`, `cargo test -p chronacle-retrieval`, `cargo test -p chronacle-extraction`, or `cargo test -p chronacle-domain`.
- **Desktop:** `pnpm -C apps/desktop test:run`; `pnpm -C apps/desktop typecheck`; Svelte analysis for changed components. Add `.feature` scenarios under `apps/desktop/tests/e2e/features/` and steps under `apps/desktop/tests/e2e/backend/steps/`; run `scripts/ci/acceptance.sh`. Mocked-IPC journeys do not replace Rust tests of actual backend contracts.
- **Website:** `pnpm -C apps/website test:run`; `pnpm -C apps/website typecheck`; `pnpm -C apps/website test:pagefind`; `pnpm -C apps/website test:e2e`. Use the suites affected by the story.
- **Before every PR:** `scripts/ci/local-pr.sh` must pass. It does not replace native UI, packaged macOS, or website browser/index checks.
- **Every story:** inject its named failure; assert visible state and stored state. Preserve draft recovery and campaign isolation. Never expose credentials in logs or fixtures.
- Commands here are future validation requirements. Audit results are recorded separately; no story is implemented by this planning package.

## REL-01: Correct source model identity

**Ready · P1 · Now · Depends:** none · **Finding:** F01.
**Story:** As a GM, I want model warnings to reflect how my books were actually indexed.
**Scope:** `apps/desktop/src-tauri/src/commands/source_commands/upload.rs`; `crates/chronacle-ingestion/src/ingestion_service/{mod.rs,tests.rs}`; provider consistency tests.

```gherkin
Scenario: Index with the selected model
  Given the active indexing model differs from Nomic
  When a PDF finishes indexing
  Then its source and chunks identify the model used for that index generation
  And the consistency check reports no mismatch for that source
```

**Also test:** multilingual and cloud identities; provider change during extraction; failed indexing must not claim successful current-model indexing; reindex clears the old identity. Capture the indexing provider once for generation metadata and vectors.

## REL-02: Migrate all searchable embeddings

**Ready · P1 · Now · Depends:** REL-01 · **Finding:** F02.
**Story:** As a GM, I want changing search models to rebuild every affected record safely.
**Scope:** provider `embedding/consistency.rs`; source `reindex.rs`; entity/session embedding services; rule embedding and retrieval queries.

```gherkin
Scenario: Complete a model migration
  Given books, entities, sessions, and compiled rules were embedded with model A
  When I select model B and complete reindexing
  Then every searchable record uses model B
  And no retrieval request compares model-B queries with model-A vectors
```

**Also test:** notes-only collections; one failed record; retry after partial migration; mixed-model startup. Show remaining counts by content type. Prevent false completion and exclude incompatible content until repaired; preserve original text and notes.

## REL-03: Keep chat owned by its campaign

**Ready · P1 · Now · Depends:** none · **Finding:** F03.
**Story:** As a GM, I want navigation to preserve the campaign that owns each question and answer.
**Scope:** desktop `commands/chat_commands.rs`, AppState chat task ownership, `lib/commands.ts`, `OracleView.svelte`, `Shell.svelte`.

```gherkin
Scenario: Change campaigns during generation
  Given an answer is streaming for campaign A
  When I select campaign B and the answer finishes
  Then B's conversation is unchanged by A's stream
  And returning to A shows its own result
```

**Also test:** leave/remount Oracle; two rapid sends; delayed terminal event; cancellation ownership; Save to Codex after switching. Correlate every token, terminal event, and save action by stable request/scope identity. Reject a second concurrent request explicitly until independent task ownership exists.

## REL-04: Create Maintenance articles in the right scope

**Ready · P1 · Now · Depends:** none · **Finding:** F04.
**Story:** As a GM, I want a Maintenance action to repair the record that produced the finding.
**Scope:** `MaintenanceView.svelte`; `Shell.svelte`; `lib/commands.ts`; lint finding DTOs and `commands/codex_commands.rs`.

```gherkin
Scenario: Repair a finding from another campaign
  Given a missing-link finding belongs to campaign A while B is active
  When I create the missing article from that finding
  Then the article is created in A's owning scope
  And the finding resolves only after that creation succeeds
```

**Also test:** shared collection ownership; cancelled/failed creation; navigation to the source entity; deleted owner. Show destination scope before editing. Never infer ownership from whichever campaign is currently selected.

## REL-05: Remove deleted-source evidence from retrieval

**Ready · P1 · Now · Depends:** none · **Finding:** F05.
**Story:** As a GM, I want deleting a book to stop answers relying on its compiled content.
**Scope:** source `query.rs`; ingestion invalidation helper; Codex/rules services; retrieval `rules_block.rs` and entity context.

```gherkin
Scenario: Delete the only supporting source
  Given an article and rule derive their evidence only from one PDF
  When I delete that PDF and ask about its content
  Then neither derived artifact supplies evidence to the answer
  And my independent GM notes remain stored
```

**Also test:** mixed-source artifacts; source removal failure; stale artifact before recompilation. Exclude affected artifacts until rebuilt from remaining evidence. Explain invalidation in the deletion UI; do not silently erase GM-authored text.

## REL-06: Report stream failures truthfully

**Ready · P1 · Now · Depends:** REL-03 · **Finding:** F06.
**Story:** As a GM, I want to know whether an answer completed, failed, or was cancelled.
**Scope:** `crates/chronacle-providers/src/llm_provider/sse.rs`; chat commands/persistence; Oracle rendering and retry.

```gherkin
Scenario: Fail after partial answer content
  Given the provider has streamed part of an answer
  When it reports an error or ends before protocol completion
  Then the partial text is visibly marked failed with Retry
  And it cannot be saved to Codex as a completed answer
```

**Also test:** OpenAI/Anthropic error payloads; empty completion; transport failure; cancellation; late terminal events; failed persistence. Preserve useful partial text with explicit status. Retry targets the original question and campaign without duplicating a completed result.

## REL-07: Repair failed entity indexing

**Ready · P1 · Now · Depends:** REL-01 · **Finding:** F07.
**Story:** As a GM, I want retrying extraction to repair entities missing from search.
**Scope:** extraction `persist.rs`, `collection_tests.rs`; entity embedding service; extraction progress/summary DTOs.

```gherkin
Scenario: Repair a failed entity embedding
  Given extraction created an entity but its embedding failed
  When extraction is retried with a working embedding provider
  Then the existing entity receives a valid current-model embedding
  And no duplicate entity is created
```

**Also test:** related entities; already-valid vectors; repeated failure. Report remaining unindexed records and expose targeted repair; do not require another full sweep merely to repair one vector.

## REL-08: Verify citations against stored passages

**Ready · P1 · Now · Depends:** REL-03, REL-05 · **Finding:** F10.
**Story:** As a GM, I want a citation to show the actual supporting passage from the correct book.
**Scope:** retrieval citation/persistence/context modules; source `citation.rs`; `lib/commands.ts`; `ruling-parse.ts`, `RulingCard.svelte`, Oracle passage UI.

```gherkin
Scenario: Inspect an unambiguous citation
  Given two collections contain different PDFs with the same filename
  When I open a citation in an answer from one collection
  Then I see its exact stored supporting passage and source identity
  And a model-written quotation is not presented as verified without a match
```

**Also test:** multiple chunks per page; missing quote; compiled-rule provenance; deleted source; legacy ambiguous citations; lookup failure/retry. Use trusted evidence IDs from retrieval, validate model references, and show an explicit unavailable/ambiguous state rather than choosing an arbitrary match.

## REL-09: Answer follow-up questions

**Ready · P2 · Next · Depends:** REL-03, REL-06 · **Finding:** F08.
**Story:** As a GM, I want a follow-up question to use the preceding exchange in the same campaign.
**Scope:** retrieval `agent_service/mod.rs` and persistence/history query helpers; capturing-provider integration tests.

```gherkin
Scenario: Refer to the preceding answer
  Given a completed exchange identified an NPC in campaign A
  When I ask "What about his faction?" in A
  Then the provider receives the preceding exchange and the new question
  And no campaign-B exchange is included
```

**Also test:** retry/current-question duplication; failed/cancelled turns; empty history. Initial bound: at most ten preceding completed exchanges and 12,000 content characters, dropping oldest whole exchanges first; keep the current question once. Label failed partial answers so they are excluded as authoritative history.

## REL-10: Retrieve evidence from long notes

**Ready · P2 · Next · Depends:** REL-02 · **Finding:** F09.
**Story:** As a GM, I want answers to find facts anywhere in my entity and session notes.
**Scope:** entity/session indexing; retrieval `context/entity.rs`, `context/format.rs`; note passage provenance and integration tests.

```gherkin
Scenario: Find a fact beyond the leading excerpt
  Given the only mention of a password occurs after character 280 of my session notes
  When I ask for that password
  Then the relevant note passage reaches the answer provider
  And the answer can identify the session that supports it
```

**Also test:** edited/deleted notes; campaign isolation; model migration; many long sessions. Retrieve passages rather than expanding every note into the prompt; preserve entity relationships and bounded context. Document and test the selected context budget in the implementation design.

## REL-11: Refresh campaign views safely

**Ready · P2 · Next · Depends:** none · **Findings:** F11, F12.
**Story:** As a GM, I want every view to show the campaign currently selected.
**Scope:** `TimelineView.svelte`, `OracleView.svelte`, `CampaignView.svelte`, `EntityManager.svelte`, Shell count loading; existing component tests.

```gherkin
Scenario: Ignore an obsolete campaign response
  Given campaign A's timeline, history, subscriptions, or counts request is delayed
  When I select B and B loads before A completes
  Then A's late response cannot change B's displayed state or actions
```

**Also test:** A → B → A; same-scope refresh; unmount; failed request; late response after delete. Reload timeline on campaign changes, clear old context, and use request generations rather than scope-string equality alone.

## REL-12: Enter the app without a local model

**Ready · P2 · Next · Depends:** none · **Finding:** F13.
**Story:** As a GM, I want to choose local/cloud setup or use notes without downloading a local model first.
**Scope:** `App.svelte`, `ModelDownload.svelte`, Settings provider controls; embedding readiness commands and tests.

```gherkin
Scenario: Skip an unavailable local download
  Given this installation has no cached local model and download is unavailable
  When I choose cloud setup or continue to notes
  Then the app opens the selected workflow
  And AI actions explain any remaining setup requirement
```

**Also test:** offline first run; unsupported local runtime; listener registration rejection; download retry; cached model. Do not treat an uncached/no-op provider as ready for indexing. Catch listener setup failures and clean up previous listeners on retry.

## REL-13: Save provider configuration atomically

**Ready · P2 · Next · Depends:** none · **Finding:** F14.
**Story:** As a GM, I want failed configuration changes to leave my working provider intact.
**Scope:** `SettingsView.svelte`, `lib/commands.ts`; settings and provider-reconfiguration services/commands.

```gherkin
Scenario: Reject a failed provider change
  Given a working provider configuration
  When saving or constructing its replacement fails
  Then the previous stored and active configuration remains usable
  And I see an actionable failure instead of a success message
```

**Also test:** one field write fails; cloud embedding configuration; rapid repeated saves; restart after failure. Validate/apply one configuration through IPC; distinguish configuration applied from an actual tested network connection. Never return/log decrypted secrets as verification evidence.

## REL-14: Refresh the library after upload

**Ready · P2 · Next · Depends:** REL-11 · **Finding:** F15.
**Story:** As a GM, I want a successfully indexed PDF to appear immediately in its collection.
**Scope:** Shell upload completion; Campaign source cache/counts; Oracle empty-state readiness.

```gherkin
Scenario: Upload into an already expanded collection
  Given an empty collection is expanded
  When its first PDF finishes indexing
  Then the book appears without navigation or reload
  And the book total and applicable Oracle guidance update
```

**Also test:** upload from Oracle; failed upload; other campaigns' collections; duplicate completion event. Count all applicable books independently of panel expansion. Base answer readiness on active subscriptions and indexing state, not any source anywhere in the library.

## REL-15: Make edit and wiki actions work

**Ready · P2 · Next · Depends:** none · **Finding:** F16.
**Story:** As a GM, I want Edit details and linked entity names to open their advertised destinations.
**Scope:** Campaign editing; SessionLogView/SessionList/SessionRow callback chain; EntityManager note previews; WikiText.

```gherkin
Scenario: Open a campaign editor from its summary
  Given Manage campaigns is collapsed
  When I choose Edit details
  Then the correct editor becomes visible with its name field focused
```

**Also test:** mouse/keyboard wiki activation from session and entity previews; deleted targets; unsaved drafts retained during navigation. Never render an enabled entity button without a working destination.

## REL-16: Show session load and delete errors

**Ready · P2 · Next · Depends:** none · **Finding:** F17.
**Story:** As a GM, I want failed session operations to explain the problem and allow a safe retry.
**Scope:** `SessionLogView.svelte`, `SessionRow.svelte`; existing list/entity-load/delete tests.

```gherkin
Scenario: Retry a failed session list
  Given a campaign contains saved sessions
  When loading them fails
  Then I see a load error with Retry instead of an empty-campaign message
  And retry can load the sessions without losing retained drafts
```

**Also test:** linked-entity failure then reopen/retry; failed delete leaves row and drafts; campaign switch while retry runs. Keep a failed linked-entity load retryable rather than marking it permanently loaded.

## REL-17: Distinguish extraction outcomes

**Ready · P2 · Next · Depends:** REL-06, REL-07 · **Finding:** F17.
**Story:** As a GM, I want extraction to distinguish new content, existing content, empty evidence, and failures.
**Scope:** extraction parser, collection sweep/summary; extraction commands/progress DTOs; Oracle ExtractionCard presentation.

```gherkin
Scenario: Report a partially failed extraction
  Given one extraction batch succeeds and another returns malformed JSON
  When extraction ends
  Then I see successful counts and the failed batch count
  And retry targets failed work instead of claiming no passages were found
```

**Also test:** valid empty result; all entities already exist; relationships-only changes; cancellation; embedding failure. Count matched/existing/created/failed work separately. Do not persist malformed output as a successful empty extraction.

## REL-18: Complete campaign deletion cleanup

**Ready · P2 · Next · Depends:** REL-05 · **Finding:** F18.
**Story:** As a GM, I want deleting a campaign to remove its owned data without damaging shared material.
**Scope:** campaign/collection deletion services and command orchestration; BlobStore cleanup; rules/sessions/messages/dependent edges; vault export enumeration.

```gherkin
Scenario: Delete owned content and preserve shared content
  Given a campaign owns books, rules, sessions, and messages and subscribes to a shared collection
  When I confirm deletion of the campaign and its owned collection
  Then its owned records and PDF blobs are removed or visibly pending retryable cleanup
  And the shared collection and other campaigns remain intact
```

**Also test:** convert-owned-collection option; injected blob/database failure; repeat deletion; orphan rule vault export; pending drafts. Retain enough cleanup identity to retry after parent deletion; never report complete cleanup while known work remains.

## REL-19: Close manual overlays after navigation

**Ready · P2 · Next · Depends:** none · **Finding:** F19.
**Story:** As a reader, I want selecting a manual destination to reveal that article immediately.
**Scope:** website `ManualDrawer.svelte`, `ManualSidebar.svelte`, `SearchDialog.svelte`; `tests/e2e/manual.spec.ts`.

```gherkin
Scenario: Navigate from a manual overlay
  Given an article's mobile contents drawer or search dialog is open
  When I select another article or a fallback link
  Then the destination article is visible without an open overlay
  And keyboard focus moves to the destination heading
```

**Also test:** article-to-article and overview routes; same-page anchor; back navigation; cancelled navigation; Escape returns focus to opener. Run against the static build in both languages, including 390px and desktop widths.

## REL-20: Retry manual search initialization

**Ready · P2 · Next · Depends:** none · **Finding:** F20.
**Story:** As a reader, I want manual search to recover after a temporary loading failure.
**Scope:** website `lib/search/{SearchDialog.svelte,pagefind.ts}` and their tests; website browser suite.

```gherkin
Scenario: Recover search without reloading the page
  Given the first search-module initialization failed
  When connectivity recovers and I retry search
  Then initialization runs again and search returns results
  And the current article and language remain unchanged
```

**Also test:** loader rejection; invalid module; initialization rejection; repeated retries; close while retrying. Reset rejected promises at adapter and dialog boundaries, prevent stale results, and retain usable fallback links.

## REL-21: Explain installation and review German copy

**Ready · P3 · Next · Depends:** none · **Finding:** F21.
**Story:** As a new user, I want installation instructions that tell me which package to choose and what to do next.
**Scope:** paired manual installation/provider/quick-start articles; README packaging matrix; German proofreading markers.

```gherkin
Scenario: Select and install a release package
  Given I know my operating system and processor
  When I follow the installation article
  Then I can identify the matching package and its installation steps
  And Flatpak instructions explain installation and update limitations
```

**Delivery slices:** first update English installation and its German counterpart; then review German articles in section-sized batches. Explain Apple Silicon/Intel and x86_64/aarch64. Match current UI labels; remove a proofreading marker only after that article is reviewed. Verify links and technical meaning; do not claim all 38 reviewed from a mechanical replacement.

## REL-22: Fix keyboard and touch interactions

**Ready · P3 · Next · Depends:** REL-08 · **Findings:** F21, F22, F23.
**Story:** As a keyboard, touch, or IME user, I want controls to work without accidental submission or lost context.
**Scope:** Oracle key handling/passage UI; RulingCard disclosure; mobile SiteHeader links; component/browser accessibility tests.

```gherkin
Scenario: Confirm composed text without sending
  Given I am composing text in Oracle with an IME
  When I press Enter to confirm composition
  Then the question remains in the composer and is not submitted
```

**Also test:** normal Enter sends; Shift-Enter inserts a line; citation expanded state/controls relationship; passage focus and return; mobile navigation targets at least 44×44 CSS pixels without overlap at 320px/390px. Verify screen-reader labels and keyboard-only citation lookup/retry.

## REL-23: Make retrieval evaluation enforceable

**Ready · P1 · Alongside · Depends:** none for harness repair; REL-02/10 before final expanded baseline · **Finding:** Q01.
**Story:** As a maintainer, I want retrieval checks to measure real work and fail when required evidence is missing.
**Scope:** `apps/desktop/src-tauri/tests/retrieval_recall.rs`; `docs/phase-1-retrieval-eval.md`; provisioned evaluation CI job.

```gherkin
Scenario: Refuse a false evaluation success
  Given evaluation is explicitly requested but a required model or PDFium is missing
  When the evaluation command runs
  Then it fails with the missing prerequisite identified
  And it does not report a passing recall measurement
```

**Also test:** nonzero evaluated queries; record count/model/fixture identity; provisioned successful run. Use `cargo test -p Chronacle --features rocksdb --test retrieval_recall -- --ignored --nocapture`. Expand toward 50 representative queries, including multilingual, long notes, distractor collections, and citations. Record measured results and decide thresholds explicitly; do not reuse the historical 100% as a current result.

## REL-24: Run website journeys in CI

**Ready · P2 · Alongside · Depends:** none · **Finding:** Q02.
**Story:** As a maintainer, I want broken built search, navigation, or accessibility to fail before release.
**Scope:** `.github/workflows/ci.yml`; CI scripts; website package scripts and Pagefind/Playwright suites.

```gherkin
Scenario: Reject a broken published-manual journey
  Given a built manual has broken language-scoped search or overlay navigation
  When its CI checks run
  Then the relevant browser or built-index check fails
  And its report identifies the failing journey
```

**Also test:** static preview, mobile drawer, translations, storage, accessibility, no-JavaScript fallback. Add `pnpm -C apps/website test:pagefind` and `pnpm -C apps/website test:e2e` to an owned required job; install browser dependencies explicitly. Document whether that job is included in local-pr or is an additional gate.

## REL-25: Verify native macOS Quit

**Ready · P1 · Alongside · Depends:** none · **Finding:** Q03.
**Story:** As a Mac GM, I want normal Quit to protect retained drafts regardless of how I initiate it.
**Scope:** macOS packaged-app validation, `macos_exit.rs` protocol, release workflow; ADR-013 verification evidence.

```gherkin
Scenario: Cancel a native Quit with an unsaved draft
  Given a packaged macOS app contains an unsaved draft
  When I initiate Quit through Cmd-Q, Dock Quit, or AppleScript and choose Cancel
  Then the app remains open with the draft intact
  And a later confirmed Quit exits exactly once
```

**Also test:** repeated Quit during one decision; window close; failed/cancelled authorization; clean exit. Exercise each initiator separately on a native packaged build. Store logs and explicit results; mark the release blocked if required native verification was not run. Unit/source-string checks and Linux UI E2E do not satisfy this story.

## REL-26: Reconcile project status

**Ready · P3 · Alongside · Depends:** none · **Finding:** D01.
**Story:** As a contributor, I want project status to identify actual remaining work without rebuilding completed features.
**Scope:** `AGENTS.md`; `docs/architecture.md` phase checklists; historical handovers; README/toolchain descriptions; roadmap links.

```gherkin
Scenario: Choose work from current project status
  Given vault sync, basic campaign editing, timelines, graphs, and draft recovery exist
  When I read project guidance
  Then those capabilities are not described as unimplemented
  And unfinished behavior links to a current story or explicit deferred decision
```

**Also check:** campaign name editing versus remaining slug/vault semantics; contradictory ingestion checkpoint claims; Node toolchain description versus mise/Docker/CI. Preserve historical records as dated history, distinguish implementation presence from verification, and keep architecture decisions authoritative.

## REL-27: Skip unchanged extraction work

**Needs spec · P2 · Later · Depends:** REL-07, REL-17 · **Work:** R01.
**Story:** As a GM, I want repeated extraction to avoid paying for unchanged books.
**Scope:** extraction collection sweep, persisted extraction fingerprints, progress preview, campaign extraction commands.

```gherkin
Scenario: Repeat an unchanged extraction
  Given a source completed extraction with the same content, model, and prompt identity
  When I run extraction again without forcing a rebuild
  Then unchanged batches make no extraction-model calls
  And failed or changed work remains eligible for processing
```

**Spec first:** fingerprint/version storage, explicit force behavior, work estimate, cancellation checkpoints. Test changed PDF/model/prompt, partial retry, and shared collections. Keep entity-index repair available independently of extraction calls.

## REL-28: Select sources for one question

**Needs spec · P2 · Later · Depends:** REL-05, REL-08 · **Work:** R02.
**Story:** As a GM, I want to exclude selected books for one answer without changing campaign subscriptions.
**Scope:** Oracle source selector, chat request, collection/chunk/rule/entity provenance filters.

```gherkin
Scenario: Exclude a book from one answer
  Given my campaign subscribes to books A and B
  When I exclude B for one question
  Then B's raw and derived evidence is absent from that request
  And campaign subscriptions are unchanged
```

**Spec first:** default/reset behavior, mixed-source derived artifacts, GM-note scope, and visible selected evidence. Test excluded-only facts and stable citation identities; filtering PDF chunks alone is insufficient.

## REL-29: Find saved chat messages

**Needs spec · P2 · Later · Depends:** REL-03, REL-06 · **Work:** R03.
**Story:** As a GM, I want to find an old question or answer within my campaign.
**Scope:** SurrealQL full-text history index/query; scoped IPC search; Oracle result navigation.

```gherkin
Scenario: Find an older campaign answer
  Given campaigns A and B contain the same search term
  When I search saved history while A is selected
  Then I see only A's matching saved messages
  And selecting a result opens that exchange without sending a question
```

**Spec first:** pagination, result ordering, failure-status display, and selected-message navigation. Include no-campaign behavior explicitly. This slice does not delete history or add threads; those need a separate retention decision.

## REL-30: Export a session as Markdown

**Needs spec · P2 · Later · Depends:** REL-16 · **Work:** R04.
**Story:** As a GM, I want to export one session's saved notes to a portable Markdown file.
**Scope:** session export service/IPC, save dialog, BlobStore/filesystem boundary, session UI and manual.

```gherkin
Scenario: Export saved session notes
  Given a session has a saved title, date, and notes
  When I export it as Markdown to a chosen destination
  Then the file contains those saved fields
  And existing files are not overwritten without explicit confirmation
```

**Spec first:** draft-save/discard choice, filename/content format, link treatment, overwrite/error recovery. First slice exports saved notes without AI summarization. Generated summaries and PDF layout are separate features. Export is GM-facing and makes no player-safe claim.

## REL-31: Restore a soft-deleted record

**Needs spec · P2 · Later · Depends:** REL-18 · **Work:** R05.
**Story:** As a GM, I want to restore an accidentally soft-deleted record without overwriting newer vault text.
**Scope:** deleted-record listing/restore commands, entity/session/rule services, vault reconcile/conflict rules, restore UI.

```gherkin
Scenario: Restore without losing vault edits
  Given a soft-deleted record has a managed file containing newer edits
  When I explicitly restore the record
  Then it becomes available in the correct scope and search after repair
  And newer file content is preserved or presented as a conflict
```

**Spec first:** eligible record types, discoverable trash, name/scope collisions, indexing repair, vault-file reconciliation, and permanent-delete distinction. Test restore without a connected vault and restore after the owning campaign is gone. Never automatically resurrect a file-deleted record during ordinary sync.
