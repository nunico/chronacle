# Changelog

All notable changes to Chronacle are documented here.

## [0.3.0] - 2026-07-22

This release makes entity identity and unresolved wikilinks much easier to
manage, while strengthening the release and persistence checks around the
desktop application.

### Added

- Added alternate names for entities and rule entries, including round-trip
  support through Markdown vault sync.
- Added fuzzy name matching that can resolve wikilinks by normalized names or
  alternate names, with reviewable aliases for uncertain matches.
- Added cross-table duplicate detection and an entity merge flow that keeps
  relationships, notes, and aliases intact.
- Added a maintenance view for naming conflicts, including real entity names,
  scrolling findings, conflict resolution, and stale-article compilation
  progress.
- Added graph actions for unresolved wikilinks, including creating a new
  article directly from a finding.
- Added full internationalization for English, German, French, and Spanish,
  with automatic OS-locale detection, persisted display-language selection,
  complete translation catalogs, interpolation, and live UI updates.
- Added language-aware Oracle responses: supported-language messages are
  answered in their detected language, with the configured UI language as the
  fallback for ambiguous messages.
- Added multilingual local retrieval with the E5 Base embedding mode,
  language-specific document/query prefixes, model identity checks, download
  progress, and explicit re-indexing safeguards when switching models.
- Added shared accessible UI controls covering buttons, progress bars, form
  fields, dialogs, and status badges, and migrated the repeated application
  surfaces to use them.
- Added shared pull-request quality gates and an optimized cached local gate
  covering backend, frontend, and acceptance validation.

### Changed

- Improved wikilink findings so unresolved references can be reviewed and
  acted on without leaving the graph workflow.
- Tuned duplicate matching with campaign evidence and deterministic matching
  behavior.
- Made the RocksDB-enabled desktop suite part of release validation and
  hardened persistence and shutdown tests.
- Added translation-catalog completeness tests and end-to-end coverage for
  locale selection, multilingual Oracle responses, embedding-mode changes,
  and re-indexing behavior.

### Fixed

- Prevented soft-deleted entities from being selected during name resolution
  and alias handling.
- Preserved alternate names and relationship notes during entity updates and
  merges, including empty-field cases.
- Fixed maintenance-row spinner and removal animations, including reduced
  motion behavior.
- Fixed bundled ONNX Runtime resolution for local embedding deployments.
- Stabilized watcher, sidecar cleanup, acceptance, and frontend test output.

## [0.2.0] - 2026-06-08

- See the repository history for the initial desktop release and Phase 2
  campaign, notes, retrieval, and vault-sync work.
