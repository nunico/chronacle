# Reliability and UX roadmap

**Planning date:** 2026-10-22 · **Baseline:** `f194470` · **Status:** Proposed; no fixes implemented.

**Start here:** [REL-01: Correct source model identity](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-01-correct-source-model-identity).

**Choose work:** use this page. **Implement:** open a story. **Check evidence:** use the [audit](audits/2026-10-22-project-audit.md).

## Now — trustworthy answers and correct campaign ownership

Finish these before adding search, export, or cloud features.

| Order | Story                                                                                                                    | Deliverable                                       | Depends on     |
| ----- | ------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------- | -------------- |
| 1     | [REL-01](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-01-correct-source-model-identity)                  | Accurate source embedding metadata                | —              |
| 2     | [REL-02](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-02-migrate-all-searchable-embeddings)              | Complete, recoverable model migration             | REL-01         |
| 3     | [REL-03](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-03-keep-chat-owned-by-its-campaign)                | Campaign-owned chat requests and streams          | —              |
| 4     | [REL-04](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-04-create-maintenance-articles-in-the-right-scope) | Correct Maintenance destination scope             | —              |
| 5     | [REL-05](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-05-remove-deleted-source-evidence-from-retrieval)  | Deleted PDFs stop supplying derived evidence      | —              |
| 6     | [REL-06](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-06-report-stream-failures-truthfully)              | Explicit failed, cancelled, and completed answers | REL-03         |
| 7     | [REL-07](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-07-repair-failed-entity-indexing)                  | Retry repairs missing entity vectors              | REL-01         |
| 8     | [REL-08](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-08-verify-citations-against-stored-passages)       | Citations resolve to actual stored evidence       | REL-03, REL-05 |

**Exit checks:** two distinct models migrate cleanly; delayed campaign-A work cannot change campaign B; deleted-source-only evidence is absent; partial failures expose Retry and cannot be saved as completed knowledge.

## Next — usable daily workflows

| Story                                                                                                                 | Deliverable                                        | Depends on     |
| --------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------- | -------------- |
| [REL-09](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-09-answer-follow-up-questions)                  | Bounded conversation context                       | REL-03, REL-06 |
| [REL-10](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-10-retrieve-evidence-from-long-notes)           | Relevant passages from long notes                  | REL-02         |
| [REL-11](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-11-refresh-campaign-views-safely)               | Fresh timelines, history, subscriptions, counts    | —              |
| [REL-12](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-12-enter-the-app-without-a-local-model)         | Recoverable first-run provider choice              | —              |
| [REL-13](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-13-save-provider-configuration-atomically)      | Truthful, atomic provider configuration            | —              |
| [REL-14](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-14-refresh-the-library-after-upload)            | Uploaded books appear immediately                  | REL-11         |
| [REL-15](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-15-make-edit-and-wiki-actions-work)             | Working campaign edit and wiki navigation          | —              |
| [REL-16](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-16-show-session-load-and-delete-errors)         | Session errors with targeted recovery              | —              |
| [REL-17](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-17-distinguish-extraction-outcomes)             | Empty, existing, partial, and failed extraction    | REL-06, REL-07 |
| [REL-18](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-18-complete-campaign-deletion-cleanup)          | Complete, retryable deletion cleanup               | REL-05         |
| [REL-19](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-19-close-manual-overlays-after-navigation)      | Manual destinations visible after navigation       | —              |
| [REL-20](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-20-retry-manual-search-initialization)          | Search recovers without page reload                | —              |
| [REL-21](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-21-explain-installation-and-review-german-copy) | Actionable installation help; reviewed German copy | —              |
| [REL-22](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-22-fix-keyboard-and-touch-interactions)         | Accessible citations, touch targets, IME input     | REL-08         |

**Exit checks:** exercise upload → ask → inspect citation → edit notes → switch campaigns → return → recover one injected failure. Complete the manual journey at desktop and mobile widths.

## Run alongside Now and Next — verification and planning hygiene

| Story                                                                                                           | Deliverable                                               | Schedule                                  |
| --------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- | ----------------------------------------- |
| [REL-23](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-23-make-retrieval-evaluation-enforceable) | Provisioned retrieval benchmark that cannot silently skip | Start with REL-01; expand after REL-02/10 |
| [REL-24](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-24-run-website-journeys-in-ci)            | Built-index and website browser checks in CI              | Before declaring REL-19/20 release-ready  |
| [REL-25](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-25-verify-native-macos-quit)              | Packaged macOS Quit/Cancel/confirm evidence               | Before the next release                   |
| [REL-26](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-26-reconcile-project-status)              | Accurate status and resolved checklist drift              | Start immediately                         |

## Later — product improvements

These require a small feature spec before implementation. Do not combine them with repair PRs.

| Story                                                                                                     | First slice                                                | Prerequisite   |
| --------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- | -------------- |
| [REL-27](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-27-skip-unchanged-extraction-work)  | Extraction fingerprints, preview, and unchanged-run skip   | REL-07, REL-17 |
| [REL-28](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-28-select-sources-for-one-question) | One-query source selection across raw and derived evidence | REL-05, REL-08 |
| [REL-29](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-29-find-saved-chat-messages)        | Campaign-scoped full-text history search                   | REL-03, REL-06 |
| [REL-30](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-30-export-a-session-as-markdown)    | User-selected session Markdown export                      | REL-16         |
| [REL-31](superpowers/specs/2026-10-22-reliability-user-stories.md#rel-31-restore-a-soft-deleted-record)   | Explicit restore with vault conflict protection            | REL-18         |

## Parked decisions

- **Checkpoint/resume:** measure large-book retry cost first; then design checkpoints and staged index replacement.
- **PDF export:** follow Markdown export only when a concrete PDF layout is specified.
- **Clear/new chat:** decide retention semantics before adding destructive history controls.
- **Player-safe export:** requires passage-level GM-secret detection; do not revive the reverted whole-record flag.
- **Cloud/mobile:** separate architecture programme after desktop reliability gates pass; no HTTP layer in the desktop app.
- **Reranking:** decide from the expanded benchmark; the old twelve-query result is insufficient evidence for current retrieval quality.
- **Cancellation atomicity / KNN filter ordering:** run [audit probes H01–H02](audits/2026-10-22-project-audit.md#unconfirmed-probes) before scheduling fixes.

## Delivery rules

1. Pick one story. Split it when it crosses independently testable contracts; retain the story ID on each slice.
2. Read its evidence and verify the defect still exists. A source-confirmed finding is not a recorded native reproduction.
3. Add the specified failing regression and executable Gherkin acceptance before production edits (ADR-011).
4. Preserve Tauri IPC, dependency traits, embedding identity, and existing draft protections. New dependencies need existing approval or an ADR.
5. Run focused tests, then `scripts/ci/local-pr.sh` successfully before creating a PR. Native and website checks listed by a story remain additional obligations.
6. Record commands/results and close the story only when its acceptance passes. Update this roadmap and audit disposition together.

## Current status

- **Done:** investigation and this planning package; frontend checks listed in the audit.
- **Not done:** every REL story; no production fixes are included in this planning commit.
- **Already present:** vault sync, basic campaign editing, timeline, graph, and substantial draft recovery. Do not rebuild them from stale checklists.
- **Architecture:** [architecture.md](architecture.md) remains authoritative for stack and ADR decisions. This roadmap supplies the new delivery order.
