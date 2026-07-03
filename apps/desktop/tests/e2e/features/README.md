# Acceptance features (ADR-011)

Gherkin `.feature` files here are executable: `bddgen` (playwright-bdd)
generates Playwright tests into `tests/e2e/.features-gen/` (gitignored),
run by the `bdd` Playwright project. Step definitions live in
`../backend/steps/`.

Run locally: `pnpm -C apps/desktop run e2e:backend`

## Conventions

- One `.feature` per feature area; scenario text is copied from the
  design spec's BDD section.
- Steps drive the frontend against the shared Tauri IPC mock
  (`../backend/ipc-mock.ts`) — the same harness as the hand-written
  backend specs.
- **Backend-only scenarios** (service-layer behaviour with no UI
  surface, e.g. schema rules or scope validation) cannot execute
  through this harness. They ship instead as Rust integration tests in
  `apps/desktop/src-tauri/tests/`, named to mirror the Gherkin scenario
  (e.g. `relation_between_two_regular_collections_is_rejected`), and the
  spec's scenario list is the source of truth for both.
