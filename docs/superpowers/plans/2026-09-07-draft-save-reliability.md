# Draft and Save Reliability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Retain correctly scoped Oracle, entity, session, and rule-note drafts
for the running app, serialize acknowledged saves, expose recovery, and prevent
normal close from silently discarding at-risk work.

**Architecture:** Add a pure revisioned draft state module and an app-scoped
Svelte coordinator with one save lane per persistence target. Editor components
remain adapters for explicit or blur-save interaction, while shared status and
close components provide localized accessible feedback. Rust changes only make
the existing rule-note command return a truthful saved-record acknowledgment.

**Tech Stack:** Svelte 5 runes, TypeScript, Tauri 2 IPC/window API, Vitest,
Testing Library, Playwright + playwright-bdd, Rust, SurrealDB in-memory tests,
tauri-driver.

---

## Execution Protocol

All production edits, test edits/execution, and reviews are performed by
subagents. The coordinator assigns the ownership below sequentially; no two
agents write the same file concurrently. Every slice follows this gate:

1. Test engineer confirms/adds the observable contract and reports a failing
   test for the intended behavioral reason.
2. Implementer reproduces that red test, writes the minimum coherent code, and
   reports green focused tests.
3. Test engineer independently runs focused and affected regression checks.
4. Specification reviewer compares code, design, and Gherkin and approves or
   returns concrete findings.
5. Only after specification approval, code/architecture reviewer assesses
   correctness, DDD boundaries, accessibility, and regressions.
6. Implementer fixes substantiated findings; test engineer reruns affected
   checks; both reviewers re-review. Repeat until accepted.

Every report includes changed files, exact commands/results, unresolved concerns,
and `git diff --stat`. Do not commit, push, merge, or create a PR.

## File Map and Ownership

**Acceptance/test-engineer-owned:**

- `apps/desktop/tests/e2e/features/draft-save-reliability.feature`
- `apps/desktop/tests/e2e/backend/steps/draft-save-reliability.steps.ts`
- `apps/desktop/tests/e2e/backend/ipc-mock.ts`
- `apps/desktop/tests/e2e/ui/draft-close.e2e.mjs`

**Slice 1 implementer-owned:**

- Create `apps/desktop/src/lib/drafts/draft-state.ts`
- Create `apps/desktop/src/lib/drafts/draft-state.test.ts`
- Create `apps/desktop/src/lib/drafts/draft-coordinator.svelte.ts`
- Create `apps/desktop/src/lib/drafts/draft-coordinator.svelte.test.ts`
- Create `apps/desktop/src/components/SaveStatus.svelte`
- Create `apps/desktop/src/components/SaveStatus.test.ts`
- Modify `apps/desktop/src/lib/i18n/messages.ts`
- Modify `apps/desktop/src/lib/i18n/locales/de.ts`
- Modify `apps/desktop/src/lib/i18n/locales/fr.ts`
- Modify `apps/desktop/src/lib/i18n/locales/es.ts`

**Slice 2 implementer-owned:**

- Modify `apps/desktop/src/shell/Shell.svelte`
- Modify `apps/desktop/src/shell/Shell.test.ts`
- Modify `apps/desktop/src/views/OracleView.svelte`
- Modify `apps/desktop/src/views/OracleView.test.ts`
- Modify `apps/desktop/src/components/EntityManager.svelte`
- Modify `apps/desktop/src/components/EntityManager.test.ts`
- Modify `apps/desktop/src/components/EntityForm.svelte`
- Modify `apps/desktop/src/components/EntityForm.test.ts`
- Modify `apps/desktop/src/views/CampaignView.svelte`
- Modify `apps/desktop/src/views/CampaignView.test.ts`
- Modify `apps/desktop/src/lib/i18n/messages.ts` for promotion-failure copy
- Modify `apps/desktop/src/lib/i18n/locales/de.ts` for promotion-failure copy
- Modify `apps/desktop/src/lib/i18n/locales/fr.ts` for promotion-failure copy
- Modify `apps/desktop/src/lib/i18n/locales/es.ts` for promotion-failure copy

**Slice 3 implementer-owned:**

- Modify `apps/desktop/src/views/SessionLogView.svelte`
- Create `apps/desktop/src/views/SessionLogView.test.ts`
- Modify `apps/desktop/src/components/SessionList.svelte`
- Modify `apps/desktop/src/components/SessionRow.svelte`
- Modify `apps/desktop/src/components/SessionRow.test.ts`
- Modify `apps/desktop/src/components/RulesPanel.svelte`
- Modify `apps/desktop/src/components/RulesPanel.test.ts`
- Modify `apps/desktop/src/lib/commands.ts`
- Modify `apps/desktop/src-tauri/src/commands/codex_commands.rs`
- Modify `crates/chronacle-extraction/src/codex_service/rules.rs`
- Modify `crates/chronacle-extraction/src/codex_service/rules_tests.rs`

**Slice 4 implementer-owned:**

- Create `apps/desktop/src/lib/drafts/window-close.ts`
- Create `apps/desktop/src/lib/drafts/window-close.test.ts`
- Create `apps/desktop/src/components/CloseDraftsDialog.svelte`
- Create `apps/desktop/src/components/CloseDraftsDialog.test.ts`
- Modify `apps/desktop/src/shell/Shell.svelte`
- Modify `apps/desktop/src/shell/Shell.test.ts`

**Documentation owner:**

- Create `apps/website/src/content/manual/en/notes-and-sessions/saving-and-recovery.md`
- Create `apps/website/src/content/manual/de/notizen-und-sitzungen/speichern-und-wiederherstellen.md`
- Modify the related Oracle, entity-note, session-log, and rule-note articles
  under `apps/website/src/content/manual/{en,de}/`
- Modify `apps/website/src/lib/content/registry.test.ts`

## Task 1: Lock the Executable Acceptance Contract

**Owner:** Test engineer. Do not edit production files.

- [ ] Confirm that `draft-save-reliability.feature` contains the scenarios in
      the design verbatim: Oracle view/campaign/no-campaign retention; existing/new
      entity retention; record isolation; baseline reversion; entity/session/rule
      failure and retry; edit-during-save; navigation-during-save; rapid saves;
      create-r1/edit-r2 promotion without duplicate Create; navigation away and
      remount during pending Create; Create/list interleaving where the backend
      row arrives before acknowledgment, including recoverable refusal when that
      listed destination is already at risk and the clean-after-newer-Update race;
      obsolete same-context manager responses rejected after a replacement
      manager's acknowledgment;
      exact-match untouched projection absorption; destination convergence when
      the source has no pending revision, including row/preview/form/status
      projection from the retained destination and exact Create/Update counts;
      explicit source/destination resolution with stable focus after both
      choices; active-Create abandonment with disabled/described destructive
      actions and no second Create; hidden-new-draft keep/replace from a link, including safe
      initial focus, Escape, and opener focus restoration;
      closed-form row status; delayed backend validation scoped to its record;
      frontend validation reset on record change; unavailable target after list
      reload; targeted discard; discard blocked during an active write; keyboard
      Retry with focus handoff on success and focus retention on repeated ordinary
      or promotion failure; native close; and
      close/discard blocked while a backend write is active.
- [ ] Extend `ipc-mock.ts` with deterministic test controls exposed only in the
      browser harness:

  ```ts
  interface DraftReliabilityControls {
    hold(
      command:
        | "create_entity"
        | "update_entity"
        | "update_session"
        | "update_rule_notes",
    ): void;
    rejectNext(command: string, error: unknown): void;
    commitPendingCreate(): string;
    resolveNext(command: string): void;
    activeWrites(command: string): number;
    maxConcurrentWrites(command: string): number;
    persisted(command: string, id: string): unknown;
  }
  ```

  Seed Campaign A/B, entities Mira/Torvin, one session, World Guide/Initiative,
  and ordinarily mutate the mock's persisted state only when a write resolves.
  `commitPendingCreate` is the one explicit two-phase control: it persists the
  held Create and returns its ID without resolving the frontend promise, so a
  subsequent list can observe the record before the delayed acknowledgment is
  delivered. Resolving that committed write must not apply it twice. This lets
  steps assert returned UI and persisted records, not merely calls.

- [ ] Correct the acceptance harness setup ordering: install the IPC mock,
      initialize deferred controls and seeded persisted state, and expose a
      readiness handshake before `page.goto` starts application boot. Do not reset
      fixture state after navigation. Add a harness regression test that delays
      setup and proves the first frontend command cannot race ahead of the seed.
      This is test infrastructure, not product draft policy.

- [ ] Implement `draft-save-reliability.steps.ts` with role/label locators and
      deferred controls. Keep `@native-close-contract` excluded from mocked browser
      execution and bind it in Task 5's tauri-driver check.
- [ ] Run the red contract:

  ```bash
  pnpm -C apps/desktop e2e:backend -- --grep "Preserve work"
  ```

  Expected red: scenarios fail because drafts disappear/leak, status/retry is
  absent, writes overlap, or close interception is not implemented. Report the
  first failure for each behavior, not only `bddgen` output.

## Task 2: Implement Pure Draft Rules and Save Coordination

**Owner:** Slice 1 implementer.

- [ ] Write failing `draft-state.test.ts` cases for all identities and
      transitions. Use explicit fixtures:

  ```ts
  const scope = entityScope("camp-a", "npc", "mira");
  const d0 = createDraft(scope, "entity:npc:mira", { notes: "saved" });
  const d1 = revise(d0, { notes: "new" });
  const a1 = beginAttempt(d1, 1);
  const d2 = revise(a1.draft, { notes: "newer" });
  expect(acknowledge(d2, a1.attempt, { notes: "new" })).toMatchObject({
    value: { notes: "newer" },
    baseline: { notes: "new" },
    revision: 2,
  });
  expect(statusOf(acknowledge(d2, a1.attempt, { notes: "new" }))).toBe(
    "pending",
  );

  const coincident = acknowledge(d2, a1.attempt, { notes: "newer" });
  expect(coincident).toMatchObject({
    value: { notes: "newer" },
    baseline: { notes: "newer" },
    revision: 2,
    acknowledgedRevision: 1,
    unacknowledgedRevision: 2,
  });
  expect(isContentDirty(coincident)).toBe(false);
  expect(statusOf(coincident)).toBe("pending");
  ```

  Cover Oracle no-campaign, campaign isolation, stable new UUID, content-based
  dirty clearing against a baseline known before the edit,
  wrong-scope/wrong-attempt acknowledgment, failure, retry, and targeted
  discard. Reject cyclic, sparse, non-finite, non-plain-object, and otherwise
  unsupported values. Prove that mutating an adapter input after `createDraft`,
  `revise`, or `acknowledge` cannot change the stored value and that recursively
  frozen values obtained from the public API cannot bypass `revise`. Assert that
  every complete record returned by `createDraft`, `revise`, `beginAttempt`,
  `acknowledge`, `rejectAttempt`, discard, clean refresh, and identity remap is
  itself runtime-frozen, as are its nested attempt and values. Also prove
  that a subsequent edit away from and deliberately back to the now-known
  coincident baseline clears the pending marker without claiming a backend
  acknowledgment. Pass malformed canonical content with a mismatched attempt and
  assert `acknowledge` returns the exact unchanged record without inspecting that
  content; the test fixture should throw if any property or key is read.

- [ ] Write failing coordinator tests with manually controlled promises. Assert
      `maxConcurrentWrites(target) === 1`, three requests persist first then newest,
      an old completion never replaces current input, targets are captured, and
      navigation-like absence of subscribers does not cancel state. Assert the
      coincident-equality draft remains included in `atRiskCount()` until its own
      acknowledgment, explicit discard, or later edit back to the already-known
      baseline. Assert `open` rejects a different target for an existing scope and
      leaves that draft unchanged; a current-context authoritative load refreshes
      baseline and value for a fully clean scope; and pending, queued, saving, or
      failed scopes retain their local state. Assert scope discard is blocked while
      that scope's write is active, whole-coordinator discard is blocked while any
      write is active, a queued but unstarted request can be discarded, and discard
      becomes available after settlement for any remaining newer revision. Assert
      `listByPrefix` returns only the requested campaign/kind records in a frozen
      read-only collection and cannot mutate coordinator state. Assert every
      record returned from `open`, `get`, `revise`, `listByPrefix`, save
      settlement, clean refresh, discard, and Create remap is a runtime-frozen
      container whose nested content is frozen. Assert
      `tryRemapAfterCreate` moves a settled successful r1 to the returned
      scope/target, preserves r2 and its pending status, and atomically replaces an
      already-open destination only when its target and immutable value exactly
      match the Create acknowledgment, it is fully clean with no active or queued
      work, and its provenance has no acknowledged authority newer than the Create
      attempt. Assert authority generation is captured when each write begins, not
      when its response arrives, and survives acknowledgment plus later clean
      `open` refresh. Start Create A, acknowledge a same-content or divergent
      Update B to the exact returned destination, then deliver A's acknowledgment;
      B's later-started generation must remain newer even though B is clean. With
      no pending source revision, assert promotion converges by preserving B and
      installing the source redirect. With pending source r2, assert both records
      remain and `destination-conflict` stays at risk until explicit
      `keep-destination` or `keep-source` resolution. The first choice discards
      only r2 and converges; the second preserves r2 as pending at the exact
      returned Update target. Neither resolution invokes a writer. Assert
      missing/unsettled sources and dirty, pending, saving, failed, queued, or
      target-mismatched destinations return a structured blocked result without
      changing either scope. Assert the
      compatibility `remapAfterCreate` delegates to the structured operation,
      returns the draft on success, and throws for established callers when the
      result is blocked. Assert a blocked result stores
      a reactive promotion issue, counts it as at-risk, keeps a clean acknowledged
      source discoverable without subscribers, and retries the local transition
      without calling a writer. Assert `atRiskCount` counts the source only once
      when its draft is also unsaved. Assert success or targeted source discard
      clears only that issue. Assert it reactively records
      `sourceScope -> destinationScope`, `resolveScope`
      returns the destination (and otherwise its input), and resolution never
      changes a draft target. Assert targeted discard through either name affects
      only the resolved draft and removes its redirects; `discardAll` clears all
      redirects. Resolve malformed canonical content for an applicable active
      attempt and assert it becomes a retryable failed record with the current
      value and saved baseline intact.
- [ ] Run red:

  ```bash
  pnpm -C apps/desktop test:run src/lib/drafts/draft-state.test.ts \
    src/lib/drafts/draft-coordinator.svelte.test.ts
  ```

  Expected: missing modules/types.

- [ ] At the post-implementation review checkpoint, run the focused regression
      tests added for record-shell immutability and acknowledgment validation
      order before changing production:

  ```bash
  pnpm -C apps/desktop test:run src/lib/drafts/draft-state.test.ts \
    -t 'freezes every returned draft record shell across its lifecycle transitions'
  pnpm -C apps/desktop test:run src/lib/drafts/draft-state.test.ts \
    -t 'ignores .* for a stale attempt before normalization'
  pnpm -C apps/desktop test:run src/lib/drafts/draft-coordinator.svelte.test.ts \
    -t 'freezes every record shell exposed while coordinating the draft lifecycle'
  pnpm -C apps/desktop test:run src/lib/drafts/draft-coordinator.svelte.test.ts \
    -t 'turns an invalid canonical response into a retryable failure without losing newer edits'
  ```

  Expected red: the first and third commands fail because record containers are
  mutable; the second fails because malformed stale canonical content is
  normalized before applicability is checked. The fourth names the existing
  applicable-malformed recovery contract and must remain green throughout the
  fix. Every filter matches at least one test; do not treat a zero-test run as
  evidence.

- [ ] Implement the pure interfaces in `draft-state.ts`:

  ```ts
  export type DraftScalar = null | boolean | number | string;
  export type DraftValue =
    | DraftScalar
    | readonly DraftValue[]
    | { readonly [key: string]: DraftValue };
  export type DraftSnapshot<T extends DraftValue> = T extends DraftScalar
    ? T
    : T extends readonly (infer Item extends DraftValue)[]
      ? readonly DraftSnapshot<Item>[]
      : { readonly [Key in keyof T]: DraftSnapshot<T[Key]> };
  export type DraftStatus = "saved" | "pending" | "saving" | "failed";
  export interface DraftAttempt<T extends DraftValue> {
    readonly id: number;
    readonly scope: string;
    readonly target: string;
    readonly revision: number;
    readonly value: DraftSnapshot<T>;
  }
  export interface DraftRecord<T extends DraftValue> {
    readonly scope: string;
    readonly target: string | null;
    readonly baseline: DraftSnapshot<T>;
    readonly value: DraftSnapshot<T>;
    readonly revision: number;
    readonly acknowledgedRevision: number;
    readonly lastAcknowledgedAttemptId: number;
    readonly unacknowledgedRevision: number | null;
    readonly inFlight: DraftAttempt<T> | null;
    readonly error: string | null;
  }
  ```

  Export typed scope constructors, `createDraft`, `revise`, `beginAttempt`,
  `acknowledge`, `rejectAttempt`, `discard`, `refreshCleanDraft`,
  `remapDraftIdentity`, `isContentDirty`, and `statusOf`.
  Validate that values are normalized acyclic JSON-like data containing only
  null, booleans, finite numbers, strings, dense arrays, and plain objects.
  Reject unsupported input before changing state. Deep-clone and recursively
  freeze every baseline, value, and attempt payload on ingress, including
  applicable canonical acknowledgments. Construct every result through one
  private record freezer so the complete `DraftRecord` container and nested
  `DraftAttempt` are runtime-frozen; TypeScript `readonly` is not sufficient.
  `refreshCleanDraft` and `remapDraftIdentity` are the narrow transitions needed
  by coordinator list reconciliation and Create promotion, so the coordinator
  does not spread records to change lifecycle state. Getters therefore expose
  deeply immutable records without defensive mutable copies.
  Compare snapshots deterministically; never use input-event flags. `revise` may
  clear `unacknowledgedRevision` when the new value returns to the baseline known
  at the start of that edit. An older acknowledgment may update `baseline`, but
  must preserve a newer `unacknowledgedRevision` even when its canonical value
  equals the current value; only the newer revision's acknowledgment or explicit
  discard resolves it. `acknowledge` must compare current in-flight scope,
  target, attempt ID, and revision before reading or snapshotting canonical
  content. Return the exact unchanged record for an inapplicable attempt. Throw a
  typed validation error only when an applicable canonical value is malformed;
  the coordinator catches it and applies `rejectAttempt` to preserve work as a
  recoverable failure.

- [ ] Implement `DraftCoordinator` with a `SvelteMap`, target-keyed lanes, and:

  ```ts
  type CreatePromotionResult<T extends DraftValue> =
    | {
        outcome: "promoted";
        resolution: "source-promoted" | "destination-converged";
        draft: DraftRecord<T>;
      }
    | {
        outcome: "blocked";
        reason:
          | "missing-source"
          | "source-not-settled"
          | "destination-target-mismatch"
          | "destination-at-risk"
          | "destination-newer-acknowledgment"
          | "destination-conflict";
      };

  interface CreatePromotionIssue {
    sourceScope: string;
    destinationScope: string;
    destinationTarget: string;
    reason: Exclude<
      CreatePromotionResult<DraftValue>,
      { outcome: "promoted" }
    >["reason"];
  }

  interface DraftCoordinatorApi {
    open<T extends DraftValue>(
      scope: string,
      target: string | null,
      baseline: T,
      origin?: "editor" | "authoritative-list",
    ): DraftRecord<T>;
    revise<T extends DraftValue>(scope: string, value: T): DraftRecord<T>;
    requestSave<T extends DraftValue>(
      scope: string,
      writer: (value: DraftSnapshot<T>) => Promise<T>,
    ): Promise<void>;
    canDiscard(scope: string): boolean;
    discard(scope: string): "discarded" | "blocked-active-save" | "missing";
    canDiscardAll(): boolean;
    discardAll(): "discarded" | "blocked-active-save";
    hasActiveWrites(): boolean;
    atRiskCount(): number;
    listByPrefix<T extends DraftValue>(
      prefix: string,
    ): readonly DraftRecord<T>[];
    resolveScope(scope: string): string;
    tryRemapAfterCreate<T extends DraftValue>(
      sourceScope: string,
      destinationScope: string,
      destinationTarget: string,
    ): CreatePromotionResult<T>;
    remapAfterCreate<T extends DraftValue>(
      sourceScope: string,
      destinationScope: string,
      destinationTarget: string,
    ): DraftRecord<T>;
    getCreatePromotionIssue(
      sourceScope: string,
    ): CreatePromotionIssue | undefined;
    retryCreatePromotion<T extends DraftValue>(
      sourceScope: string,
    ): CreatePromotionResult<T>;
    resolveCreatePromotion<T extends DraftValue>(
      sourceScope: string,
      choice: "keep-destination" | "keep-source",
    ): CreatePromotionResult<T>;
  }
  ```

  `open` compares the target, including `null`, before doing anything. A mismatch
  throws and preserves the existing record. Entity list reconciliation passes
  `authoritative-list`; ordinary editor opens retain the default. With the same
  target, an authoritative adapter load replaces baseline and value only when the
  scope is fully clean and has no queued request; reset that clean record's
  revision bookkeeping but never erase its retained acknowledged-authority
  generation. Pending/saving/failed/unacknowledged scopes remain unchanged.
  Use the pure `refreshCleanDraft` transition for the accepted refresh and
  `remapDraftIdentity` after Create eligibility is established. Store only
  transition-produced, runtime-frozen records; `open`, `get`, `revise`,
  `listByPrefix`, settlement, discard, refresh, and remap must never expose a
  mutable record shell. Do not use object spread in the coordinator to mutate
  `DraftRecord` lifecycle or identity fields.
  A request during a write replaces only the queued request for that target.
  After settlement, start it only if its scope/revision is still dirty. Do not
  store IPC closures as durable draft data; the current adapter supplies Retry.
  Treat a fulfilled writer value as runtime-untrusted even though the writer is
  generically typed. Apply acknowledgment through the pure transition so stale
  or mismatched attempts are ignored before canonical validation. Catch
  validation failure from an applicable attempt, pass it through
  `rejectAttempt`, and retain the current content/baseline with Retry for the
  same scope and target. A scope discard cancels its queued request but returns
  `blocked-active-save` while that scope has an active attempt. `discardAll`
  returns `blocked-active-save` while any lane is active. Only after settlement
  may callers discard remaining unsaved content against the now-current baseline.
  `listByPrefix` returns a frozen read-only array of immutable matching records.
  Reuse the existing monotonic attempt ID as a save-start authority generation;
  on each applicable acknowledgment, retain it in a private scope-provenance map.
  A later clean authoritative-list `open` may refresh content but cannot clear or
  decrease that generation. Use the pure draft-value equality rule (export it
  narrowly from `draft-state.ts` if needed), never object-reference equality,
  JSON stringification, or input-event flags.

  `tryRemapAfterCreate` atomically moves a successfully acknowledged, settled new
  draft to an unoccupied existing-record scope/target. If the destination is
  already open, replace it only when its target exactly equals `destinationTarget`,
  it is fully clean, its value exactly equals the source Create canonical
  baseline, it has authoritative-list provenance, and it has no acknowledged
  authority generation newer than the source Create attempt. Preserve the source
  baseline, current value, revisions, and newer unacknowledged edit. If a clean
  exact-target destination is divergent or newer and the source has no pending
  revision, preserve the destination and atomically converge the source redirect
  onto it. If the source is pending, retain both and return
  `destination-conflict`. Return the other discriminated blocked results, with no
  mutation, for a missing/unsettled source or an at-risk or target-mismatched
  destination.

  `remapAfterCreate` remains a compatibility wrapper that delegates to the
  structured operation, returns the resulting draft for either successful
  resolution, and throws on a blocked result for established callers/tests;
  EntityManager uses only the structured operation. On promotion or convergence,
  store the redirect in a private reactive `SvelteMap`. On a blocked result,
  retain one reactive `CreatePromotionIssue` keyed by source scope; it holds
  identities/reason, not draft content, and makes `atRiskCount` include the source
  once. `retryCreatePromotion` reuses those captured identities and performs no
  write. `resolveCreatePromotion` accepts only a retained
  `destination-conflict`: `keep-destination` removes the source and converges,
  while `keep-source` replaces the fully clean exact-target destination and keeps
  the source's post-Create revision pending for a later explicit Update. Recheck
  target and active/queued safety atomically. Neither choice calls a writer.
  Successful promotion/convergence, explicit resolution, and targeted source
  discard clear only that issue; `discardAll` clears all promotion issues and
  provenance.
  `resolveScope` is a read-only identity query; mutation/save methods do not
  silently derive a persistence target from an alias. Targeted discard resolves
  an alias only for local discard, affects that one destination draft, and removes
  redirects to it. `discardAll` clears redirects.

- [ ] Rerun the four exact post-implementation review-checkpoint commands after
      the minimum fix. Expected green: every command executes at least one test
      and passes. Then run both complete draft suites together to catch lifecycle
      regressions:

  ```bash
  pnpm -C apps/desktop test:run src/lib/drafts/draft-state.test.ts \
    src/lib/drafts/draft-coordinator.svelte.test.ts
  ```

- [ ] Add localized `drafts.*` keys for the agreed strings **Unsaved changes**,
      **Saving…**, **Saved**, **Couldn't save**, **Retry**, **Discard changes**,
      retained-this-session helper text, unavailable-target detail, and the
      `drafts.waitForSavingBeforeDiscard` SaveStatus copy **Wait for saving to
      finish before discarding changes** in English source plus
      German/French/Spanish catalogs. This slice owns only editor/`SaveStatus` copy;
      Task 5 adds and tests close-dialog-specific copy.
- [ ] Implement `SaveStatus.svelte` with polite status announcements, alert
      failure, and real Retry/Discard buttons. Component tests assert names, roles,
      localized rendering, Enter activation, Discard disabled with an accessible
      reason while the scope writes, re-enabled after settlement, and no Saved
      label for an unacknowledged recovered/local draft.
- [ ] Run green and quality:

  ```bash
  pnpm -C apps/desktop test:run src/lib/drafts src/components/SaveStatus.test.ts
  pnpm -C apps/desktop exec svelte-check --tsconfig ./tsconfig.json
  npx -y @sveltejs/mcp svelte-autofixer apps/desktop/src/components/SaveStatus.svelte --svelte-version 5
  ```

  Expected: all focused tests pass; typecheck/autofixer report no errors.

- [ ] Complete the independent test/spec/code-review loop from Execution
      Protocol before starting Task 3.

## Task 3: Retain Oracle and Explicit Entity Drafts

**Owner:** Slice 2 implementer after test engineer adds focused red tests.

- [ ] Test engineer adds/red-runs component cases: Oracle survives unmount-like
      view navigation, isolates Campaign A/B/no-campaign, and never sends on restore;
      entity drafts survive view/kind/record changes, new draft creates no row,
      baseline reversion clears status, not-found retains content, retry targets Mira,
      clean record reload adopts the latest canonical value, at-risk reload keeps
      the local value, mismatched target open is rejected, and discard leaves
      Oracle/Torvin untouched. Add the deferred-save case where Discard changes is
      disabled during Mira's active write and enabled after settlement for a newer
      remaining revision. Add red cases for Create r1/edit r2 promotion without a
      duplicate Create; navigation/remount while Create is pending for both clean
      r1 and newer-r2 completion, including an entity list that loaded before the
      returned ID existed. Add the inverse deterministic interleaving: commit the
      held Create without resolving its promise, navigate/remount so `getEntities`
      lists and reconciles the returned ID as a clean destination, then deliver the
      acknowledgment. Assert atomic promotion preserves r2 as Unsaved, switches to
      Save/Update, and produces exactly one Create. Pre-open an at-risk and a
      target-mismatched destination in focused cases; assert neither is absorbed,
      both drafts remain, **Created, but needs attention** survives remount, and
      Retry performs no Create. Also cover hidden-new-draft Keep and Discard/replace;
      closed-form pending/saving/failed row text; unavailable-target reload and
      reopen; a delayed Mira backend `VALIDATION` while Torvin is active; and
      frontend required-name validation with no IPC that clears when switching
      Mira -> Torvin -> Mira without clearing Mira's draft. Add a deterministic
      deferred-promise regression that holds Mira Update, unmounts EntityManager,
      remounts it, completes `getEntities` with Mira's old row while Update is
      active, acknowledges Update with canonical content, and only then clicks
      the already-rendered row. Assert that the click preserves the acknowledged
      canonical baseline and Saved state. Complete a second, later entity load
      with newer canonical content and assert that this fresh load refreshes the
      clean scope. Add a separate adapter-liveness regression: hold manager A's
      initial list request, unmount A, let replacement manager B load and
      acknowledge newer Mira content, then resolve A's old response with the same
      campaign/kind and earlier Mira content. Assert A neither reconciles the
      app-lifetime coordinator nor publishes presentation, while B's acknowledged
      content remains Saved. This is an EntityManager coordination test, not a
      coordinator state-machine test. Add the clean-after-newer-authority ordering: hold Create A,
      commit and reconcile its returned ID, save Update B through that destination
      and acknowledge it, then deliver A's acknowledgment. With source r2, assert
      B remains visibly Saved and persisted, A remains available with
      **Created, but needs attention**, and no duplicate Create or automatic Update
      occurs. Repeat with B acknowledging content equal to A to prove content
      equality does not erase authority. With no source r2, give B a newer name
      and notes preview, then assert the client scope converges onto B without
      replacing B: the row, preview, selected form, Saved status, and persisted
      record all retain B; the redundant source presentation disappears; and the
      harness reports exactly one Create and one Update. For blocked outcomes,
      assert the source and destination remain separately discoverable with their
      own content and status rather than being collapsed by the Create response.
      Use the exact no-source-r2 component title in the focused filter below.
      Cover both explicit conflict choices and assert that keyboard activation
      moves focus to the stable same-record editor: the retained destination for
      **Keep saved record**, and the promoted pending draft for **Keep my draft**.
      Also assert the create-from-link dialog initially focuses **Keep editing**,
      Escape preserves the draft and restores focus to its invoking Create
      control. For ordinary and promotion Retry, assert success transfers focus to
      the same record's stable editor/status target before the button disappears;
      repeated failure or blocked promotion retains/restores focus on Retry. Hold
      Create and exercise its Cancel/discard path: the destructive action must be
      disabled with a visible explanation, the client draft must remain, and no
      second Create may start before the original settles. Assert
      state and rendered content, not only IPC calls.
- [ ] Run red:

  ```bash
  pnpm -C apps/desktop test:run src/views/OracleView.test.ts \
    src/components/EntityForm.test.ts src/components/EntityManager.test.ts \
    src/shell/Shell.test.ts
  ```

  Expected: retention/status/retry assertions fail against component-local state;
  the deferred ordering case fails because `openEdit` reapplies the old cached
  row after the acknowledgment.

- [ ] Capture focused red evidence for the reconciliation race:

  ```bash
  pnpm -C apps/desktop test:run src/components/EntityManager.test.ts \
    -t 'keeps a save acknowledgment over a pre-ack list row but accepts a later clean reload'
  ```

  Expected: FAIL after the final row click because the old pre-acknowledgment
  notes replace the canonical Update response (or the Saved state is lost).

- [ ] Capture focused red evidence for obsolete adapter settlement:

  ```bash
  pnpm -C apps/desktop test:run src/components/EntityManager.test.ts \
    -t 'ignores a stale list response from an unmounted manager after a newer save acknowledgment'
  pnpm -C apps/desktop e2e:backend --grep \
    'Ignore an old entity list after its view has closed'
  ```

  Expected: FAIL because the unmounted first manager still shares the replacement
  manager's campaign/kind, so its late response reconciles the app-lifetime
  coordinator or publishes old presentation after the newer acknowledgment. The
  component and BDD filters must each select exactly one test/scenario.

- [ ] Capture a separate red checkpoint for Create/list promotion ordering:

  ```bash
  pnpm -C apps/desktop test:run src/lib/drafts/draft-coordinator.svelte.test.ts \
    src/components/EntityManager.test.ts \
    -t 'atomically promotes an acknowledged create over a clean exact-target destination|counts a blocked create promotion as at-risk and retries it without absorbing unsafe work|promotes a remounted Create when its committed record was reconciled before acknowledgment|keeps Create work and exposes an action when promotion meets an at-risk destination'
  pnpm -C apps/desktop e2e:backend --grep \
    'Finish creating when the saved record appears before acknowledgment|Recover when the listed destination has unsaved work'
  ```

  Expected: FAIL because the current coordinator rejects every occupied
  destination and the adapter lets that rejection escape after Create already
  succeeded. The observable journey must fail before any second Create occurs.
  The filters must select four focused unit/component tests and two BDD scenarios;
  a zero-test or one-scenario selection is not valid red evidence.

- [ ] Capture the clean-after-newer-authority red checkpoint independently:

  ```bash
  pnpm -C apps/desktop test:run src/lib/drafts/draft-coordinator.svelte.test.ts \
    src/components/EntityManager.test.ts -t \
    'does not promote an older create acknowledgment over a destination with newer acknowledged authority|preserves a newer acknowledged destination when an older Create acknowledgment arrives|keeps destination presentation authoritative when a settled Create acknowledges last'
  pnpm -C apps/desktop e2e:backend --grep \
    'Preserve newer saved authority when Create acknowledges last|Keep converged destination presentation when Create acknowledges last|Keep my draft after a newer destination is acknowledged'
  ```

  Expected: FAIL because fully clean content is currently considered disposable;
  the delayed Create acknowledgment replaces Update B, incorrectly promotes
  pending source r2, or the adapter replaces converged destination presentation
  with the older Create response. The unit/component filter must select three
  tests, including the named no-source-r2 case asserting the newer destination
  row, notes preview, selected
  form, Saved status, persisted content, removed source presentation, and exactly
  one Create plus one Update. The BDD filter must select all three journeys rather
  than reporting zero, one, or two tests.

- [ ] Capture focused accessibility red evidence:

  ```bash
  pnpm -C apps/desktop test:run src/components/EntityManager.test.ts -t \
    'keeps Create work and exposes an action when promotion meets an at-risk destination|initially focuses the safe Keep editing action in create-from-link confirmation|cancels create-from-link confirmation with Escape and restores invoking control focus|moves keyboard focus to the stable Save action while an ordinary retry saves|restores keyboard focus to Retry when an ordinary retry remains actionable'
  pnpm -C apps/desktop e2e:backend --grep \
    'Cancel replacing a hidden new draft with the keyboard|Move focus safely after promotion Retry|Keep focus when promotion Retry remains blocked|Move focus safely when inline Retry disappears|Keep focus on inline Retry when recovery still fails'
  ```

  Expected: five selected tests fail for focus landing on a destructive/default
  control, body, or a removed Retry button. The filter count must be exactly five;
  zero or fewer than five selected tests are not evidence.

- [ ] Capture the active-Create abandonment and conflict-choice focus red
      checkpoint:

  ```bash
  pnpm -C apps/desktop test:run src/components/EntityManager.test.ts -t \
    'blocks replacing an active Create until it settles without discarding or duplicating work|preserves a newer acknowledged destination when an older Create acknowledgment arrives|keeps my pending Create draft without writing until an explicit Update save'
  pnpm -C apps/desktop e2e:backend --grep \
    'Wait for active creation before replacing a new entity draft|Preserve newer saved authority when Create acknowledges last|Keep my draft after a newer destination is acknowledged'
  ```

  Expected: three selected component tests and three BDD scenarios fail because
  an active Create can still expose an operable destructive path, or a successful
  conflict resolution removes the focused action without establishing a stable
  same-record focus target. No test may accept `document.body`, another row, or a
  second Create request.

- [ ] Create one `DraftCoordinator` in `Shell.svelte` and pass it to Oracle,
      direct entity managers, CampaignView, and later session/rule adapters. Do not
      create coordinators per view.
- [ ] Replace Oracle's local `input` ownership with the applicable
      `oracleScope(activeCampaignId)`. On input call `revise`; on intentional
      submission clear only that scope. Campaign change must swap values without a
      send. Show retained-this-session copy for nonempty drafts.
- [ ] Adapt EntityManager/Form to open typed value objects from canonical
      `GraphNode`, update the coordinator on every edit, and use existing/new scope
      constructors. Use `listByPrefix('entity-new:{campaign}:{kind}:')` as the sole
      retained-new registry and enforce at most one result; do not keep app-lifetime
      draft identity in component/module state. Make New resume it. Normalize
      optionals to the documented `DraftValue` boundary before opening or revising.
      Pass only current-context, stale-guarded loads to `open`; its clean refresh
      updates canonical data while any at-risk draft remains locally authoritative.
- [ ] Make list reconciliation the single authority boundary in
      `EntityManager.svelte`. Give each mounted manager a liveness guard that its
      cleanup invalidates. After both that guard and the captured campaign/kind
      guard accept a completed `getEntities` response, call `draftCoordinator.open` with
      `authoritative-list` provenance for every backend row before assigning
      `entities`. Clean scopes refresh then; at-risk scopes retain their
      coordinator snapshots. Treat the resulting
      `PresentationRow.node` as display/navigation data only: `openEdit` must read
      the existing coordinator scope and call `open(scope, target, rowBaseline)`
      only when `get(scope)` shows that the scope was never reconciled. Never
      reapply an old cached row after a save acknowledgment made the scope clean.
      A later accepted list completion may refresh that clean scope through the
      same load-time path. A response from an unmounted or superseded manager is
      rejected before coordinator reconciliation and presentation assignment,
      even when its campaign/kind equal the replacement's. Keep the existing
      campaign/kind stale guard; this is adapter coordination, not a coordinator
      state-machine transition. Add no list request versions, backend generation fields, service, or dependency.
      The coordinator-local save-attempt authority generation is the only new
      ordering metadata.
- [ ] Rerun the focused reconciliation test after the minimum implementation:

  ```bash
  pnpm -C apps/desktop test:run src/components/EntityManager.test.ts \
    -t 'keeps a save acknowledgment over a pre-ack list row but accepts a later clean reload'
  ```

  Expected: PASS; the held Update acknowledgment remains visible and Saved after
  clicking the cached row, and the later clean list completion refreshes it.

- [ ] Rerun the obsolete-adapter test after the minimum liveness guard:

  ```bash
  pnpm -C apps/desktop test:run src/components/EntityManager.test.ts \
    -t 'ignores a stale list response from an unmounted manager after a newer save acknowledgment'
  pnpm -C apps/desktop e2e:backend --grep \
    'Ignore an old entity list after its view has closed'
  ```

  Expected: PASS; the old instance performs neither coordinator reconciliation
  nor presentation assignment, and the replacement's acknowledged content stays
  visible and Saved. Do not add coordinator state or backend versions to make
  this adapter-owned test pass.

- [ ] Refactor only after both list-ordering tests are green. Keep the liveness
      predicate local and named at the EntityManager adapter boundary, remove any
      duplicated post-await guard branches, and rerun both focused component/BDD
      filters. At the staged specification review, require evidence that an
      obsolete same-context manager calls neither `draftCoordinator.open` nor a
      presentation assignment; at code review, reject moving component liveness
      into the pure state machine.

- [ ] Keep Save/Create explicit. During Create r1, disable Create and ignore a
      duplicate create intent while fields remain editable. On acknowledgment,
      call `tryRemapAfterCreate` with the returned ID. If r2 was entered meanwhile,
      preserve it as pending in the existing-record scope, change the action to
      Save, and call `updateEntity(returnedId, ...)`; if no r2 exists, show Saved.
      Expose the assigned identity once through the structured promotion outcome,
      derive presentation from its retained draft rather than blindly inserting
      the Create response, and prove no second Create occurs. Normal Update
      requests continue using coordinator coalescing.
- [ ] Handle Create promotion as a distinct local coordination result, not as a
      second persistence attempt. When the returned destination is an already-open
      fully clean, exact-value, exact-target authoritative-list projection with no
      newer acknowledgment, atomically replace it with the acknowledged source and
      preserve r2. If a clean destination is divergent or has newer acknowledged
      authority and the source has no pending r2, preserve the destination and
      converge the source redirect onto it. If r2 exists, leave both drafts
      untouched and retain a `destination-conflict` issue. Present
      **Keep saved record** and **Keep my draft** as explicit keyboard-operable
      resolution actions; call `resolveCreatePromotion` and do not write until a
      later explicit Save. For every other structured blocked result, retain the
      existing process-lifetime issue so a remounted EntityManager renders
      localized **Created, but needs attention** plus an explanation and Retry.
      Disable Create while any issue exists. Retry calls
      `retryCreatePromotion(sourceScope)` only; it must not call `createEntity` or
      `updateEntity`. Clear the issue on successful promotion/convergence, explicit
      resolution, or explicit discard of that source, and restore normal
      Save/Update behavior only after identity resolution. Add
      `drafts.createdNeedsAttention` = **Created, but needs attention** and
      `drafts.finishCreatedEntity` = **The entity was saved, but Chronacle couldn't
      finish opening it. Resolve changes for the listed record, then retry.**,
      `drafts.createdEntityConflict` = **This record was saved again while creation
      was finishing. Choose which version to keep.**,
      `drafts.keepSavedRecord` = **Keep saved record**, and
      `drafts.keepMyDraft` = **Keep my draft** to the English source catalog, with
      equivalent German/French/Spanish entries.
- [ ] After `tryRemapAfterCreate`, rebuild active selection and presentation from
      the discriminated coordinator outcome rather than inserting the Create
      response as a row. For `source-promoted`, use `result.draft` so untouched
      revision 1 is Saved and a newer source revision stays visible as Unsaved.
      For `destination-converged`, use `result.draft` as the row, notes preview,
      selected form, and status authority; retain its newer name/content and
      Saved state, remove the redundant source presentation, and do not invoke a
      writer. For `blocked`, read source and destination scopes independently and
      render both with their own content/status plus the retained issue. Do not
      run a common post-Create insertion path that can overwrite the selected
      coordinator draft or collapse distinct blocked presentations.
- [ ] Rerun the exact Create/list tests after the minimum implementation:

  ```bash
  pnpm -C apps/desktop test:run src/lib/drafts/draft-coordinator.svelte.test.ts \
    src/components/EntityManager.test.ts \
    -t 'atomically promotes an acknowledged create over a clean exact-target destination|counts a blocked create promotion as at-risk and retries it without absorbing unsafe work|promotes a remounted Create when its committed record was reconciled before acknowledgment|keeps Create work and exposes an action when promotion meets an at-risk destination'
  pnpm -C apps/desktop e2e:backend --grep \
    'Finish creating when the saved record appears before acknowledgment|Recover when the listed destination has unsaved work'
  ```

  Expected: PASS. The clean destination is replaced atomically; the unsafe
  destination retains both drafts and exposes recovery; neither path sends a
  second Create. The unit/component command must report four selected passing
  tests, and the BDD command must report two selected passing scenarios.

- [ ] Rerun the exact clean-after-newer-authority tests after the minimum
      implementation:

  ```bash
  pnpm -C apps/desktop test:run src/lib/drafts/draft-coordinator.svelte.test.ts \
    src/components/EntityManager.test.ts -t \
    'does not promote an older create acknowledgment over a destination with newer acknowledged authority|preserves a newer acknowledged destination when an older Create acknowledgment arrives|keeps destination presentation authoritative when a settled Create acknowledges last'
  pnpm -C apps/desktop e2e:backend --grep \
    'Preserve newer saved authority when Create acknowledges last|Keep converged destination presentation when Create acknowledges last|Keep my draft after a newer destination is acknowledged'
  ```

  Expected: PASS. Update B remains the saved and persisted destination; pending
  source r2 remains recoverable, the no-r2 source converges with its newer
  destination row/preview/form/Saved state intact, and explicit resolution
  affects only these two scopes. The automatic convergence journey reports
  exactly one intentional Create and one intentional destination Update, with no
  promotion-triggered write; conflict-resolution journeys report no implicit
  Update. The component command must report three selected passing tests, and the
  BDD command must report three selected passing scenarios.

- [ ] Implement and rerun accessible recovery focus. Retain the create-from-link
      invoking element, configure the dialog's initial focus to **Keep editing**,
      and let Escape close through `modalBehavior` so focus returns to that exact
      control. Await ordinary and promotion recovery operations: if the focused
      Retry disappears on success, move focus to the same record's stable first
      editor control or status anchor after rendering; if failure/conflict remains,
      retain or restore focus to Retry. Explicit promotion resolution uses the same
      successful handoff and never focuses another row: **Keep saved record**
      targets the retained destination editor, while **Keep my draft** targets the
      promoted pending-draft editor.

  ```bash
  pnpm -C apps/desktop test:run src/components/EntityManager.test.ts -t \
    'keeps Create work and exposes an action when promotion meets an at-risk destination|initially focuses the safe Keep editing action in create-from-link confirmation|cancels create-from-link confirmation with Escape and restores invoking control focus|moves keyboard focus to the stable Save action while an ordinary retry saves|restores keyboard focus to Retry when an ordinary retry remains actionable'
  pnpm -C apps/desktop e2e:backend --grep \
    'Cancel replacing a hidden new draft with the keyboard|Move focus safely after promotion Retry|Keep focus when promotion Retry remains blocked|Move focus safely when inline Retry disappears|Keep focus on inline Retry when recovery still fails'
  ```

  Expected: PASS with focus on the safe initial action, exact opener, same-record
  editor/status target, or still-actionable Retry as specified; no assertion may
  accept `document.body` as recovery. The command must report exactly five selected
  passing tests.

- [ ] Implement and rerun the active-Create abandonment and both explicit-choice
      focus contracts:

  ```bash
  pnpm -C apps/desktop test:run src/components/EntityManager.test.ts -t \
    'blocks replacing an active Create until it settles without discarding or duplicating work|preserves a newer acknowledged destination when an older Create acknowledgment arrives|keeps my pending Create draft without writing until an explicit Update save'
  pnpm -C apps/desktop e2e:backend --grep \
    'Wait for active creation before replacing a new entity draft|Preserve newer saved authority when Create acknowledges last|Keep my draft after a newer destination is acknowledged'
  ```

  Expected: PASS with three selected tests/scenarios at each layer. While Create
  is unsettled, destructive abandonment is disabled and described and the active
  request count remains one. Each conflict choice lands focus on its stable
  same-record editor before removing the action.

- [ ] In `EntityManager.svelte`, resolve the active scope through
      `draftCoordinator.resolveScope(activeDraftScope)` before every draft read,
      revision, retry, or save. Reactively adopt a new resolved destination and
      parse its returned record ID after a cross-instance Create remap; never infer
      or replace its persistence target. When the list loaded before Create
      completion and omits that ID, derive one acknowledged provisional
      presentation row from the destination draft so clean r1 stays selected as
      Saved and r2 stays selected as Unsaved with Save/Update. Do not insert that
      row into `entities`, the wikilink map, or backend state; replace it naturally
      when a later authoritative load includes the ID.
- [ ] Keep frontend required-name validation field-local: retain pending state and
      send no IPC. Treat backend `VALIDATION`, `NOT_FOUND`, and database rejection
      as failed state with persistent inline Retry of the latest content to the
      captured target; never close or demote backend validation to pending-only.
      Store field-oriented adapter feedback as `{ scope, error }` and render it
      only when `scope === resolvedActiveScope`, so Mira's delayed failure never
      appears on Torvin; reopening Mira must still render the coordinator failure
      and Retry even after remount. Add required `editingScope: string` to
      `EntityForm.svelte`, pass the resolved active scope from EntityManager, and
      clear only local `nameError` whenever that prop changes. Do not clear either
      record's draft or coordinator failure during this reset.
- [ ] Derive entity presentation rows from backend results plus exact existing/new
      campaign/kind prefix queries. Show pending/saving/failed text for closed
      drafts. If an at-risk existing draft is absent after reload, synthesize a
      clearly unavailable presentation row from its immutable snapshot; never add
      it to `entities`, wikilinks, or backend state. Reopen the retained scope
      without replacing its baseline; Retry preserves its target. Stop synthesizing
      it after explicit discard while the backend row remains absent.
- [ ] Let ordinary view/record navigation retain without dialogs. Dirty Cancel
      opens the shared `Dialog` titled Unsaved changes with Cancel initially focused
      and Discard changes destructive; clean Cancel closes directly. Reuse
      `modalBehavior` for trap/Escape/focus restoration. Keep the existing
      create-from-link replacement confirmation for an explicit competing new draft;
      its initial focus is the safe **Keep editing** action, Escape cancels, and
      focus returns to the exact Create control that invoked it, never the
      destructive action or `document.body`.
      Find that dirty/saving/failed new draft by coordinator prefix even when an
      existing form is selected. Keep reopens it unchanged; Discard and create
      removes only it, allocates a fresh identity, and seeds the requested name.
      While this scope has an active backend write, keep Cancel focused, disable
      Discard changes, and announce the localized wait-for-saving reason. Re-enable
      discard after settlement if a newer unsaved revision remains; discarding then
      restores the newly acknowledged baseline. A queued but unstarted request may
      be canceled and discarded. For an active Create, apply the same block to
      every destructive abandonment or replacement route, describe that creation
      must finish, preserve the stable client scope, keep Create disabled, and
      permit no second Create before settlement.
- [ ] Guard `loadEntities` first by component-instance liveness and then by
      captured campaign/kind scope. Cleanup invalidates the issuing manager so a
      late completion cannot reconcile the app-lifetime coordinator or populate
      presentation after unmount, even when a replacement has the same props.
- [ ] Run green:

  ```bash
  pnpm -C apps/desktop test:run src/views/OracleView.test.ts \
    src/components/EntityForm.test.ts src/components/EntityManager.test.ts \
    src/views/CampaignView.test.ts src/shell/Shell.test.ts
  pnpm -C apps/desktop typecheck
  ```

- [ ] Test engineer independently runs the Oracle/entity Gherkin scenarios and
      inspects the persisted mock state. Complete spec then code review; fix and
      repeat until both approve.

## Task 4: Make Session and Rule Autosave Reliable

**Owner:** Slice 3 implementer after test engineer red tests.

- [ ] Write/red-run SessionRow tests with deferred `updateSession`: visible
      Unsaved/Saving/Saved states, failure retains all three fields, Retry saves the
      same session, edit r2 during r1 completion remains pending even when r1's
      canonical response equals r2, r2 becomes Saved only after its own
      acknowledgment, three rapid blur requests never overlap and coalesce to
      newest, keyboard Retry success restores focus to the title before Retry is
      removed, and repeated failure retains/restores focus on the Retry action.
- [ ] Write/red-run RulesPanel equivalents for table notes, collection/record
      isolation, collapse/remount retention, rejection without unhandled promise,
      retry, stale completion, unavailable target, success focus handoff to the
      same rule textarea, and repeated-failure focus retention on Retry.
- [ ] Write/red-run `SessionLogView.test.ts` proving Campaign B never displays
      Campaign A's session/draft and stale Campaign A loading cannot win.
- [ ] Write a failing Rust service test before changing production:

  ```rust
  #[tokio::test]
  async fn update_rule_notes_returns_saved_entry_and_rejects_missing_id() {
      let db = setup_test_db().await;
      let entry = seed_rule_entry(&db, "house rules", None).await;
      let saved = update_rule_notes(&db, &entry.id, Some("roll once".into()))
          .await
          .expect("existing rule must acknowledge");
      assert_eq!(saved.notes.as_deref(), Some("roll once"));
      assert!(update_rule_notes(&db, "missing", None).await.is_err());
  }
  ```

- [ ] Run red:

  ```bash
  pnpm -C apps/desktop test:run src/components/SessionRow.test.ts \
    src/views/SessionLogView.test.ts src/components/RulesPanel.test.ts
  cargo test -p chronacle-extraction update_rule_notes_returns_saved_entry -- --nocapture
  ```

  Expected: UI states/recovery fail; Rust return type/absence behavior fails.

- [ ] Pass the shared coordinator SessionLog → SessionList → SessionRow. Open
      `session:{campaign}:{id}`, revise on input, and request save on blur. Retry
      calls the same adapter with current scoped content. Apply returned canonical
      Session as acknowledgment; update the list without overwriting a newer draft.
      Normalize session editor values to `DraftValue`. Reload sessions in a
      campaign-keyed effect with a captured-scope stale guard, so only an
      authoritative current-context result reaches `open`; clean scopes refresh
      while at-risk scopes retain their local snapshots. Await Retry; after the DOM
      settles, focus the same session title when success removes Retry or retain/
      restore focus to Retry when failure remains.
- [ ] Pass coordinator and active campaign through CampaignView → RulesPanel.
      Open `rule:{campaign}:{collection}:{id}`, retain on collapse/navigation, and
      blur-save through the rule target lane. Catch all promise rejection and render
      `SaveStatus` adjacent to the textarea. Normalize note values before they cross
      the draft boundary and pass only current-context collection loads to `open`.
      Apply the same awaited focus rule to that rule's textarea/Retry; never move
      focus to another rule row.
- [ ] Change Rust `update_rule_notes` to `UPDATE ... RETURN AFTER`, parse exactly
      one `RuleEntry`, and return an error if absent. Propagate `Result<RuleEntry,
String>` through the Tauri command and `Promise<RuleEntry>` through
      `commands.ts`. Use returned notes as the canonical baseline.
- [ ] Run green:

  ```bash
  cargo test -p chronacle-extraction update_rule_notes -- --nocapture
  cargo test -p Chronacle codex_commands -- --nocapture
  pnpm -C apps/desktop test:run src/components/SessionRow.test.ts \
    src/views/SessionLogView.test.ts src/components/RulesPanel.test.ts \
    src/views/CampaignView.test.ts
  pnpm -C apps/desktop typecheck
  ```

- [ ] Test engineer independently executes session/rule failure, race,
      navigation, rapid-save, and unavailable-target Gherkin. Complete both reviews,
      fix, rerun, and re-review.

## Task 5: Guard Normal Native Close

**Owner:** Slice 4 implementer; native test file remains test-engineer-owned.

- [ ] Test engineer writes red `window-close.test.ts`,
      `CloseDraftsDialog.test.ts`, and Shell cases using a fake port:

  ```ts
  export interface WindowClosePort {
    onCloseRequested(
      handler: (event: { preventDefault(): void }) => void,
    ): Promise<() => void>;
    destroy(): Promise<void>;
  }
  ```

  Assert clean close is not prevented; dirty/saving/failed Oracle/entity/session/
  rule states are prevented; Cancel initially has focus; Tab is trapped; Escape
  cancels and restores composer focus. With no active write, Discard and close
  clears all and calls destroy. With any active backend write, it is disabled with
  a polite localized reason, `discardAll` reports `blocked-active-save`, and destroy
  is not called. After settlement, keep the original close prevented: enable
  Discard and close if a newer/failed draft remains, or replace it with an enabled
  Close action if all risk is clean. Keep focus on Cancel until the user moves it;
  only an explicit enabled close action calls destroy. Assert no writer/chat
  function is invoked by the close flow.

  Before adding production locale entries, assert localized rendering in English,
  German, French, and Spanish for the close-dialog-specific **Discard and close**,
  **Wait for saving to finish before discarding and closing**, and **Saving
  finished. It is safe to close.** copy. Assert the completion action uses the
  already-existing `common.close` key rather than adding a duplicate `drafts.*`
  key.

- [ ] Run red:

  ```bash
  pnpm -C apps/desktop test:run src/lib/drafts/window-close.test.ts \
    src/components/CloseDraftsDialog.test.ts src/shell/Shell.test.ts
  ```

- [ ] Implement the Tauri adapter with `getCurrentWindow().onCloseRequested` and
      forced `destroy()`. Register once for Shell lifetime and always unlisten on
      teardown, including the listener-promise-after-destroy race.
- [ ] Add the close-dialog-only `drafts.discardAndClose`,
      `drafts.waitForSavingBeforeClose`, and `drafts.safeToClose` keys to the
      English source plus German/French/Spanish catalogs. Reuse the existing
      `common.close` key for the final enabled Close action.
- [ ] Implement `CloseDraftsDialog` with the existing `Dialog`/`modalBehavior`.
      Title it Unsaved changes, mark Cancel `data-autofocus`, and initially use
      Cancel and Discard and close. Derive the destructive action's disabled state from
      `coordinator.hasActiveWrites()`, expose the localized wait-for-saving message
      in a polite live region, and preserve focus on Cancel/the dialog as state
      changes. When all writes are settled, Discard clears the coordinator before
      forced destroy. If settlement removes all risk, announce that it is safe to
      close and replace the destructive action with Close; do not replay the
      prevented close or steal focus. The close path starts no save.
- [ ] Run focused green, typecheck, and Svelte autofixer.
- [ ] Test engineer adds `draft-close.e2e.mjs`: configure a non-local embedding
      backend, focus/fill Oracle, request close through the actual native window,
      assert dialog and focus, press Escape and assert composer/focus/value, request
      close again, keyboard-activate Discard and close, and assert no chat/save IPC
      happened during closing. In a second journey, hold an already-started session
      save, make a newer edit without requesting another save, and request native
      close. Assert Discard and close is disabled with its reason and destroy has not
      occurred; settle the first save, assert the newer revision remains unsaved and
      the destructive action becomes available, then activate it and confirm close
      without an extra save.
- [ ] Build and run only this native check:

  ```bash
  pnpm -C apps/desktop exec tauri build --no-bundle --features rocksdb
  xvfb-run -a pnpm -C apps/desktop exec mocha --timeout 180000 \
    tests/e2e/ui/draft-close.e2e.mjs
  ```

  Expected: pass on Linux with `tauri-driver` and WebKitWebDriver. If either tool
  is absent, report its exact command/error; component mocks do not qualify as
  native verification.

- [ ] Complete independent test/spec/code review, fixes, reruns, and re-review.

## Task 6: Document the User Contract

**Owner:** Documentation subagent after behavior is green.

- [ ] Add paired canonical manual articles, English first and then German, at
      `apps/website/src/content/manual/en/notes-and-sessions/saving-and-recovery.md`
      and
      `apps/website/src/content/manual/de/notizen-und-sitzungen/speichern-und-wiederherstellen.md`.
      Document Oracle/entity retention while Chronacle runs, campaign/record
      isolation, explicit entity Save/Discard, session/rule autosave states,
      Retry, the normal-close decision, and the accurate limitation that crash,
      force quit, or restart loses drafts. State that navigation and close never
      submit an unsent question. Keep translation keys paired and follow the
      established `/en/manual` and `/de/handbuch` link conventions.
- [ ] Cross-link and update the existing English/German Oracle, entity-note,
      session-log, and rule-note articles whose save wording is affected. In
      particular, replace any promise that blur alone means saved with the
      pending/saving/saved/failed and Retry contract. Preserve explicit German
      proofreading markers on articles that have not received human review;
      translation by this task is not grounds to remove them.
- [ ] Extend `apps/website/src/lib/content/registry.test.ts` to assert the paired
      article registration and language-local links. Run the website manual
      checks:

  ```bash
  pnpm -C apps/website test:run src/lib/content/registry.test.ts
  pnpm -C apps/website typecheck
  pnpm -C apps/website lint
  pnpm -C apps/website build
  pnpm -C apps/website test:pagefind
  ```

- [ ] Run:

  ```bash
  pnpm -C apps/desktop exec prettier --check \
    ../website/src/content/manual/en/notes-and-sessions/*.md \
    ../website/src/content/manual/en/codex/articles-and-notes.md \
    ../website/src/content/manual/de/notizen-und-sitzungen/*.md \
    ../website/src/content/manual/de/kodex/artikel-und-notizen.md \
    ../website/src/lib/content/registry.test.ts \
    ../../docs/superpowers/specs/2026-09-07-draft-save-reliability-design.md \
    ../../docs/superpowers/plans/2026-09-07-draft-save-reliability.md
  ```

  Expected: pass; format with Prettier if required, then re-check.

## Task 7: Integrated Acceptance, Review, and Final Verification

**Owner sequence:** Test engineer → specification reviewer → code/architecture
reviewer → implementer fixes → test engineer/reviewers again.

- [ ] Record the exact final state before verification:

  ```bash
  git rev-parse HEAD
  git status --short
  git diff --stat
  ```

- [ ] Test engineer runs focused deterministic suites:

  ```bash
  pnpm -C apps/desktop test:run src/lib/drafts src/components/SaveStatus.test.ts \
    src/components/CloseDraftsDialog.test.ts src/views/OracleView.test.ts \
    src/components/EntityForm.test.ts src/components/EntityManager.test.ts \
    src/views/SessionLogView.test.ts src/components/SessionRow.test.ts \
    src/components/RulesPanel.test.ts src/shell/Shell.test.ts
  cargo test -p chronacle-extraction update_rule_notes -- --nocapture
  pnpm -C apps/desktop e2e:backend -- --grep "Preserve work"
  ```

- [ ] Run frontend Svelte analysis on every modified `.svelte` file and resolve
      actionable errors:

  ```bash
  npx -y @sveltejs/mcp svelte-autofixer apps/desktop/src/views/OracleView.svelte --svelte-version 5
  npx -y @sveltejs/mcp svelte-autofixer apps/desktop/src/components/EntityManager.svelte --svelte-version 5
  npx -y @sveltejs/mcp svelte-autofixer apps/desktop/src/components/EntityForm.svelte --svelte-version 5
  npx -y @sveltejs/mcp svelte-autofixer apps/desktop/src/components/SessionRow.svelte --svelte-version 5
  npx -y @sveltejs/mcp svelte-autofixer apps/desktop/src/components/RulesPanel.svelte --svelte-version 5
  npx -y @sveltejs/mcp svelte-autofixer apps/desktop/src/shell/Shell.svelte --svelte-version 5
  ```

- [ ] Specification reviewer checks every design invariant and Gherkin outcome
      against actual code and test evidence. Code reviewer then checks lane keys,
      stale acknowledgment guards, normalized immutable snapshots, target mismatch
      rejection, clean authoritative refresh, retained save-start authority,
      exact-target/value Create promotion, no-work convergence, explicit
      two-authority conflict resolution, campaign/record isolation, truthful backend
      acknowledgment, active-write discard/close blocking, safe dialog defaults,
      recovery focus/live regions, privacy, and absence of new dependencies.
- [ ] Implementer resolves every substantiated finding without weakening tests.
      Test engineer reruns affected commands and both reviewers explicitly re-review
      the final diff.
- [ ] Test engineer runs the authoritative PR gate even though no PR is created:

  ```bash
  scripts/ci/local-pr.sh
  ```

  Expected: Backend quality, Frontend quality, and Acceptance all pass.

- [ ] Rerun the native close command from Task 5 against this same final working
      tree state. Report any platform/tool limitation exactly.
- [ ] Confirm no lockfile, secret, generated output, capability manifest,
      license, or brand asset is in `git diff --name-only`. Final report lists what
      is safe now, in-memory lifetime/close policy, shared design, exact checks,
      review findings/resolutions, and any concrete unverified native limitation.
