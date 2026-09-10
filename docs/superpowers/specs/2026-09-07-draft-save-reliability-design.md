# Draft and Save Reliability Design

**Date:** 2026-09-07
**Status:** Implemented; corrective revision after adversarial review
**Area:** `apps/desktop` editing lifecycle, Tauri IPC acknowledgments, native window close

## Outcome

A GM can move between Oracle questions, campaign notes, sessions, and reference
material without losing work. Every editable surface distinguishes work that is
unsaved, saving, saved, or blocked by an actionable failure.

This design keeps drafts for the lifetime of the running Chronacle process. It
does not persist draft text across a crash or restart. A normal close request is
therefore intercepted whenever at-risk work exists; the GM must cancel closing
or explicitly choose **Discard and close**. No asynchronous save is started
during shutdown.

## Scope

Included:

- Oracle composer text, including the no-campaign context.
- Existing and new entity forms for all entity kinds.
- Session title, date, and notes editing.
- Compiled rule-entry table notes.
- View navigation, keyboard navigation, campaign changes, record selection,
  collection panels, and normal native window closing.
- Save acknowledgment, stale completion, retry, discard, deleted targets,
  accessibility, and localized status copy.

Not included:

- Durable crash/restart recovery.
- Changes to chat submission, citations, onboarding, search, Codex compilation,
  vault conflict semantics, or campaign ownership.
- A generic workflow engine, event sourcing, database table, backend draft
  service, or general-purpose dependency for shared draft state or persistence.
  ADR-013 deliberately adds target-specific `objc2` 0.6 and `objc2-app-kit` 0.3
  dependencies solely for the macOS native Quit adapter.
- Redesigning existing explicit entity Save into autosave.
- Changing pull-request topology. The corrective work is designed as separable
  commits and review slices, but splitting the already-published PR requires
  remote branch/PR operations and remains a maintainer handoff unless explicitly
  authorized.

## Investigation and Reproduction

### Current-state flow map

| Surface          | Draft currently lives in                                                                         | Save trigger and acknowledgment                                                        | Navigation / selection behavior                                                                                                                                            | Failure / race behavior                                                                                                                                                       |
| ---------------- | ------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Oracle composer  | `OracleView.svelte` local `input`                                                                | Enter/click calls `chatSend`; unsent text has no persistence                           | `Shell.svelte` conditionally unmounts Oracle on another view, losing text. Campaign changes reuse the component, so the same text incorrectly appears in another campaign. | No draft identity. An unsent question is never intentionally submitted, but scope isolation is absent.                                                                        |
| Existing entity  | Writable `$derived` fields inside `EntityForm.svelte`; `EntityManager` holds selected `formNode` | Explicit Save; `updateEntity` returns the saved `GraphNode`                            | View/category unmount loses fields. Selecting another record reseeds fields. Campaign/kind effect closes and clears the form.                                              | Save button is not coordinated with revisions. A `NOT_FOUND` response closes the form and loses the local edit.                                                               |
| New entity       | Same form-local fields; `formNode = null`                                                        | Explicit Create; `createEntity` returns the created `GraphNode`                        | View/category unmount or another record loses fields. No backend row exists before Create.                                                                                 | The Maintenance missing-article path alone has a dirty-form confirmation. Its `initialName` is treated as the clean baseline even though no domain record exists.             |
| Session          | Writable `$derived` fields in each keyed `SessionRow`                                            | Blur of title, date, or notes calls `updateSession`, which returns the saved `Session` | Leaving Sessions unmounts rows. `SessionLogView` loads only in `onMount`, so a campaign change while the view remains mounted can show the old campaign's sessions.        | Failure is `console.error` only. Every blur starts a write, so requests overlap and an older completion can apply after newer input.                                          |
| Rule table notes | `RulesPanel.svelte` local `notesDraft` object                                                    | Blur calls `updateRuleNotes`, currently returning `void`                               | Collapse retains only while the panel is mounted. Leaving Campaign, changing relevant panels, or remounting loses it.                                                      | There is no `try/catch`, so rejection is unhandled. Writes overlap. The backend `UPDATE` returns `Ok(())` even when no record exists, so absence can be falsely acknowledged. |

### Navigation and lifecycle paths

`Shell.svelte` owns a `View` union and renders exactly one top-level view. All
rail clicks, `g` keyboard chords, `/`, timeline/entity deep links, Maintenance
links, campaign management, and campaign switching ultimately mutate `view`,
`activeCampaignId`, `pendingOpen`, or `pendingCreate`. Conditional rendering
unmounts Oracle, entity managers, Sessions, and Campaign/Rules. Within entity
managers, row clicks call `openEdit`, the New button calls `openCreate`, and a
campaign/kind effect explicitly clears the form. `CampaignView` also conditionally
mounts `RulesPanel` for an expanded collection's Rules tab.

Entity Cancel clears the form without confirming dirty work. Session and rule
editors have no Cancel/Discard control. The only dirty guard is
`blockedPendingCreate`, used when a Maintenance-origin create request would
replace an already dirty entity form. Delete confirmation and that guard use the
shared `Dialog`, whose `modalBehavior` provides initial focus, a focus trap,
Escape-to-close, and focus restoration.

The global toast store auto-dismisses success/info notices after four seconds
and keeps error toasts until dismissal. It is useful supplementary feedback but
cannot be the only recovery surface because it is detached from the affected
field. EntityManager also has a separate four-second local toast. No code
registers `getCurrentWindow().onCloseRequested` or `beforeunload`.

Entity and session command promises acknowledge completed persistence by
returning canonical saved records. Rule notes return `void`; the Rust service
does not check that a row was returned. Background session embedding,
distillation, and outbound-vault enqueueing are explicitly best effort after the
domain write and do not change whether the edit itself was acknowledged.

### Reproduction evidence

The following checks were run against commit `56d3805` before production edits:

```text
pnpm -C apps/desktop test:run
Result: 38 files passed; 313 tests passed.

pnpm -C apps/desktop test:run src/views/OracleView.test.ts \
  src/components/EntityManager.test.ts src/components/EntityForm.test.ts \
  src/components/SessionRow.test.ts src/components/RulesPanel.test.ts \
  src/shell/Shell.test.ts
Result: 6 files passed; 88 tests passed.
```

A Vite instance (`pnpm -C apps/desktop dev --host 0.0.0.0`) was driven with
Playwright Chromium and the same mocked Tauri IPC boundary used by the acceptance
suite. The scripted workflows produced:

```json
{"oracleAfterNav":"","entityAfterNav":"Aldric","newEntityAfterNav":""}
{"shownInB":"Campaign A secret","restoredInA":"Campaign A secret"}
{"visibleRecovery":0,"titleAfterNav":"Saved session","consoleLogged":true}
{"visibleRecovery":0,"noteAfterNav":"Saved rule note","unhandled":true}
```

Thus the starting findings were reproduced: view navigation loses Oracle and
entity drafts; an Oracle draft leaks into another campaign; failed session and
rule saves have no recovery UI and disappear after navigation. The session error
was logged as `Failed to update session: Error: disk full`; the rule rejection
was an unhandled `Error: database locked`.

The product-native preview reported `available: false`, so the reproduction used
the permitted Playwright fallback after that explicit unavailable result.

After the test engineer added the red Gherkin contract, this command established
the expected acceptance red state:

```text
pnpm -C apps/desktop e2e:backend -- --grep "App shell smoke|Compiled rules browsing"
Result: failed during bddgen with 94 missing steps from
the then-combined draft/save reliability feature.
```

That failure is behavioral test scaffolding, not a regression in the prior
suite. `npx -y @sveltejs/mcp svelte-autofixer` reported no issue for
`SessionRow.svelte`; for `RulesPanel.svelte` it only suggested reviewing the
intentional state-loading effect.

## Ubiquitous Language

- **Draft:** editable frontend content plus its saved baseline and lifecycle
  metadata. A draft is not domain content until a write is acknowledged.
- **Draft value:** normalized, acyclic JSON-like data: `null`, a boolean, a
  finite number, a string, a readonly array of draft values, or a readonly
  plain-object map of draft values. Editor adapters normalize optional fields
  before crossing this boundary; `undefined`, sparse arrays, non-finite numbers,
  cycles, functions, symbols, bigints, class instances, `Date`, `Map`, and `Set`
  are rejected.
- **Immutable draft record:** a `DraftRecord` whose outer container, nested
  `DraftAttempt`, baseline, current value, and save-attempt payload are all
  frozen at runtime as well as readonly in TypeScript. Every lifecycle change
  produces a new immutable record through a named transition, so a caller must
  use `revise` or another explicit transition and cannot bypass identity,
  revision, or acknowledgment accounting.
- **Editing scope:** the complete identity of the UI context that owns a draft.
  It prevents a value from appearing in another campaign, collection, record,
  or new-record form.
- **Persistence target:** the backend aggregate that a write addresses. Save
  lanes are keyed by this target even when two editing contexts can expose the
  same shared collection record.
- **Saved baseline:** latest canonical content acknowledged by the backend.
- **Content-dirty:** value inequality with the saved baseline.
- **Unacknowledged revision:** the current local revision still requiring its
  own save acknowledgment. It is distinct from content-dirty because an older
  acknowledgment can reveal canonical content equal to a newer local revision
  without acknowledging that newer revision.
- **Draft revision:** a monotonically increasing number assigned to each value
  change in one editing scope.
- **Save attempt:** one captured scope, target, revision, and immutable payload
  handed to an adapter.
- **Save acknowledgment:** successful IPC completion applicable to the exact
  currently in-flight scope, target, attempt, and revision, including the
  canonical returned value where available. Applicability is established before
  the canonical payload is inspected or snapshotted.
- **Authority generation:** a coordinator-local, monotonically increasing number
  captured when a persistence attempt actually begins. An applicable
  acknowledgment records that attempt's generation on its editing scope. This
  provenance survives later clean list refreshes and distinguishes a projection
  from content that became clean through a locally acknowledged write; it is not
  a backend version, timestamp, or Rust-domain concept.
- **Untouched list projection:** a fully clean destination created by
  authoritative list reconciliation whose exact target and immutable canonical
  value match the Create acknowledgment, and which has no local acknowledged
  authority newer than that Create attempt. It may be replaced during Create
  promotion because it is another presentation of the same persisted revision.
- **Promotion convergence:** completion of Create identity resolution by keeping
  a clean destination as authoritative, removing the redundant settled source,
  and installing the source-to-destination redirect. Convergence never changes
  backend content and is automatic only when the source has no pending revision.
- **Promotion conflict:** a recoverable Create result in which the exact-target
  destination has different or newer authority while the acknowledged source
  also contains a pending post-Create revision. Both drafts remain available
  until the GM explicitly keeps the saved destination or keeps the source draft.
- **Promotion presentation:** the EntityManager projection derived after Create
  identity resolution from the coordinator's exact outcome and retained draft
  records. The older Create response supplies the assigned identity and
  acknowledgment payload, but it is never independently authoritative for a
  row, preview, form, or save status once coordination has completed.
- **Discard:** explicit replacement of one draft's value with its saved baseline,
  or removal of one unsaved new-record draft.
- **Recoverable failure:** a rejected save or an applicable acknowledgment with
  invalid canonical content that retains draft content, identifies the target,
  and exposes Retry and relevant discard action. Invalid content attached to an
  inapplicable stale completion is ignored with that completion.
- **Release:** removal of a fully clean, inactive draft snapshot after its
  adapter no longer needs it. Release is not discard: it is permitted only when
  the value equals its acknowledged baseline, no save is active or queued, no
  failure or promotion issue exists, and no retained UI depends on the scope.
- **Deletion intent:** an immutable capture of the complete destructive target
  (campaign, kind or collection, record ID, and editing scope) made when a
  confirmation opens. Confirmation never reconstructs a target from mutable
  selection or navigation state.
- **List reconciliation:** the one-time application of a current-campaign,
  current-kind backend list row to its coordinator scope when that list request
  completes through the still-live adapter instance that issued it. A rendered
  row is presentation data after reconciliation, not a reusable authoritative
  baseline.
- **Accepted adapter request:** an asynchronous request whose captured editing
  context is still current and whose issuing adapter instance is still mounted
  and authoritative when it settles. Matching campaign and kind are necessary
  but not sufficient: a response owned by an unmounted or obsolete manager is
  never accepted.
- **Load authority snapshot:** the `lastAcknowledgedAttemptId` observed for each
  already-open scope when a list request begins. If that authority advances
  before the request settles, the older response cannot refresh that now-clean
  scope. This is adapter-local ordering evidence, not a backend version.

## Domain Boundaries

### Rules: pure frontend draft state

`apps/desktop/src/lib/drafts/draft-state.ts` owns identities, value/baseline
comparison, revision transitions, applicability of acknowledgments, discard, and
failure retention. It has no Svelte, IPC, DOM, or Tauri imports. These are
editing-domain rules, not Rust business-domain state.

Its public value boundary is recursive and readonly:

```ts
type DraftScalar = null | boolean | number | string;
type DraftValue =
  | DraftScalar
  | readonly DraftValue[]
  | { readonly [key: string]: DraftValue };
```

All creation, revision, refresh, identity-remap, and acknowledgment entry points
return a runtime-frozen `DraftRecord`; its nested attempt and draft-value data
are runtime-frozen too. The module exposes narrow pure transitions for clean
authoritative refresh and identity remap rather than a generic record mutator.
The coordinator never constructs a changed record with object spread and never
hands out a mutable internal container.

Editor-specific types are normalized into `DraftValue` before calling the
rules. Successful adapter completions remain untrusted at runtime. The rules
first compare the supplied attempt with the record's current in-flight scope,
target, attempt ID, and revision. An inapplicable completion returns the exact
unchanged record without reading, cloning, or validating its canonical payload.
Only then is an applicable payload validated and snapshotted. If that validation
fails, the coordinator converts it to the same recoverable-failure transition as
a rejected IPC write, preserving the current value and applicable attempt's
scope, target, and revision for Retry.

### Application coordination

`apps/desktop/src/lib/drafts/draft-coordinator.svelte.ts` owns the app-lifetime
map of drafts and one save lane per persistence target. A lane keeps one active
request plus an ordered queue. Repeated queued requests from the same editing
scope are removed and replaced by that scope's newest request at the tail,
carrying their waiters forward; requests from a different scope that shares the
target are never discarded as coalescing. This preserves request order and the
latest final intent while preventing overlap. Success/failure applies only to
the attempt's original scope. `Shell.svelte` creates one coordinator and passes
it to editor adapters. Navigation changes no draft state.
The coordinator reuses its monotonic attempt sequence as the authority
generation and keeps private provenance per scope. `open` can establish or
refresh authoritative-list provenance, while an applicable acknowledgment
records the attempt generation without exposing metadata to editor components.
Clean list refreshes update snapshots but never erase that acknowledged
generation. This is the minimum ordering needed for Create promotion; no backend
version column, wall clock, generic generation service, or Rust-domain state is
added.

The coordinator owns orchestration, not record construction. It stores and
returns only records produced by the pure state transitions. `open`, `get`,
`revise`, `listByPrefix`, save settlement, clean authoritative refresh, discard,
and Create remap therefore expose the same runtime-immutable record contract.
Writer adapters supply a typed promise for developer ergonomics, but the
coordinator treats its fulfilled value as untrusted at runtime and delegates
applicability and canonical validation to the rules. It catches validation only
for an applicable active attempt and records a recoverable failure; a stale
completion is a no-op and cannot create a failure on the current draft.

Deletion is coordinated separately from save-state transitions. A successful
backend delete calls the coordinator's narrow `removeAfterDelete(scope)`
operation, which cancels only that scope's queued intent and removes only that
scope. It refuses removal while the same scope has an active write. The session
adapter owns the delete request and its temporary deleting state; the pure draft
rules do not model UI confirmation or backend deletion.

The same coordination applies to entities and campaigns. Entity deletion
captures an immutable deletion intent before opening confirmation, disables
global navigation shortcuts while any modal is active, and cannot start while
that entity target has an active save. Success removes exactly the captured
scope; failure preserves it. Campaign deletion is refused while any target lane
for that campaign is active. When no write is active, confirmation identifies
all dirty, failed, queued, and unsent campaign scopes and requires explicit
campaign-wide discard before the backend delete. Success releases those scopes;
failure leaves them intact. No deletion derives kind, campaign, or record from
live selection after confirmation opens.

The coordinator is app-scoped but not an archive. Adapters call `release(scope)`
for clean, inactive scopes when rows leave the authoritative projection or an
editor unmounts with no retained work. `releasePrefix(prefix)` applies the same
eligibility rule after campaign or collection teardown. At-risk records,
redirects, and promotion issues remain retained. Rule recovery presentation
stores only the minimal rule identity and label needed to reopen the exact note
draft; compiled bodies, references, objections, and other rule content remain
authoritative adapter data and are not duplicated in an app-lifetime cache.
This bounds clean-snapshot memory and private-text retention without timers,
quotas, durable storage, or a second cache service.

The coordinator deliberately does not decide whether a component instance is
still alive. Each asynchronous editor adapter must reject settlement from an
unmounted or superseded instance before calling `open`, revising presentation,
or otherwise supplying backend authority. The coordinator outlives those
instances, so a campaign/kind equality check alone cannot make an old response
safe: an obsolete manager may share the same props as its replacement and still
settle later. This liveness rule belongs to application/adapter coordination,
not the pure draft state machine and not the Rust business domain.

Session and rule-list adapters additionally snapshot the
`lastAcknowledgedAttemptId` of their already-open exact-prefix scopes when each
load starts. At settlement, component liveness, request generation, campaign,
and collection checks run first. For each returned record, reconciliation is
skipped when its current authority is newer than the request's snapshot. This
prevents a response that began before a save acknowledgment from replacing the
newly acknowledged canonical baseline merely because the draft is clean by the
time the response arrives. A subsequent accepted request captures the new
authority and may refresh that clean scope. No wall clock, database version, or
generic request-order service is introduced.

The coordinator also exposes narrow application operations used by the entity
adapter:

```ts
type CreatePromotionResult<T extends DraftValue> =
  | {
      readonly outcome: "promoted";
      readonly resolution: "source-promoted" | "destination-converged";
      readonly draft: DraftRecord<T>;
    }
  | {
      readonly outcome: "blocked";
      readonly reason:
        | "missing-source"
        | "source-not-settled"
        | "destination-target-mismatch"
        | "destination-at-risk"
        | "destination-newer-acknowledgment"
        | "destination-conflict";
    };

interface CreatePromotionIssue {
  readonly sourceScope: string;
  readonly destinationScope: string;
  readonly destinationTarget: string;
  readonly reason: Exclude<CreatePromotionResult<DraftValue>, { outcome: "promoted" }>["reason"];
}

listByPrefix<T extends DraftValue>(prefix: string): readonly DraftRecord<T>[];
release(scope: string): "released" | "retained-at-risk" | "missing";
releasePrefix(prefix: string): number;
removeAfterDelete(scope: string): "removed" | "blocked-active-save" | "missing";
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
getCreatePromotionIssue(sourceScope: string): CreatePromotionIssue | undefined;
retryCreatePromotion<T extends DraftValue>(sourceScope: string): CreatePromotionResult<T>;
resolveCreatePromotion<T extends DraftValue>(
  sourceScope: string,
  choice: "keep-destination" | "keep-source",
): CreatePromotionResult<T>;
```

`listByPrefix` returns a frozen array of immutable draft records. It allows a
mounted entity manager to derive retained-new and unavailable-existing rows for
its campaign/kind without owning a second app-lifetime registry. It is a
read-only query, not a subscription or persistence API. `removeAfterDelete` is
called only after confirmed backend deletion succeeds; it refuses an active
same-scope write, cancels that scope's queued intent, and removes no other scope.
`tryRemapAfterCreate` is the structured, application-facing operation used by
EntityManager after a successful Create acknowledgment. The source retains the
authority generation of the Create attempt. Ordinarily the destination is
absent, so the operation atomically changes the client draft's scope and target
to the exact returned backend identity while preserving its canonical baseline,
current value, revision bookkeeping, and any newer unacknowledged edit.

A list response may instead open the returned record scope after the backend
commits Create but before its response reaches the initiating adapter. Promotion
may replace that destination only when the target matches exactly, the scope is
fully clean, its immutable value exactly matches the Create canonical value, and
its retained provenance shows no local acknowledged write newer than the Create
attempt. That is an untouched projection of the same persisted revision, not a
second authority.

If the exact-target destination has divergent or newer authority but the source
has no post-Create pending revision, the source contains no unique work. The
coordinator converges by preserving the destination, removing the redundant
source, and installing the redirect. If the source does have a pending revision,
the coordinator preserves both drafts and records a `destination-conflict`
promotion issue. This includes a destination Update whose save attempt began
after Create, even when its acknowledged content happens to equal the source;
content equality cannot acknowledge the source's revision or erase the newer
authority. An at-risk destination, mismatched target, missing source, or
unsettled source likewise returns a structured failure without changing either
draft.

`remapAfterCreate` is a compatibility wrapper for established coordinator callers
and tests. It delegates to `tryRemapAfterCreate`, returns the promoted draft on
success, and throws when the structured result is blocked. EntityManager must use
the structured operation so an expected promotion collision becomes visible and
recoverable rather than an unhandled adapter error.

The atomic eligibility check and state move are coordinator state-machine rules.
On refusal,
the coordinator retains a reactive, process-lifetime `CreatePromotionIssue`
containing identities and the reason, but no second copy of draft content. The
issue counts as at-risk work and keeps even a clean acknowledged source
discoverable after the initiating component unmounts. `atRiskCount` counts its
source scope once even when the source also has an unsaved revision. Targeted
source discard and a successful retry clear only that source's issue.
EntityManager owns application handling of a returned backend ID and the
presentation of the issue. It disables another Create and shows **Created, but
needs attention** with a persistent localized explanation: **The entity was
saved, but Chronacle couldn't finish opening it. Resolve changes for the listed
record, then retry.** Retry calls `retryCreatePromotion`, repeating only the
local transition after a transient destination risk is resolved; it never
invokes Create again. A `destination-conflict` additionally offers explicit
**Keep saved record** and **Keep my draft** actions through
`resolveCreatePromotion`. The first discards only the pending source revision and
converges onto the destination. The second preserves the source as pending,
replaces only the fully clean destination, and makes the next explicit Save an
Update to the exact returned target. Neither choice performs backend I/O. This
makes an unexpected promotion failure visible and recoverable without silently
absorbing another authority or duplicating the already-created record.

Successful remapping also writes a reactive, one-way
`sourceScope -> destinationScope` redirect in the coordinator. `resolveScope`
reads that redirect and otherwise returns its argument unchanged. It is an
identity-resolution query only: `open`, `revise`, `requestSave`, and retry never
silently follow an alias or derive a persistence target from it. EntityManager
must resolve its active client scope before reading, revising, or saving and
must then use the destination draft's already-remapped target. This prevents a
stale component from turning a client Create target into an Update target by
guesswork. Redirects are process-local, contain no draft content, and are
removed by `discardAll`; targeted discard accepts either the source alias or
resolved destination, discards only that destination draft, and removes every
redirect pointing to it. A new draft may then reuse neither the old client ID
nor the old redirect.

### Adapters

- Oracle, EntityForm/EntityManager, SessionRow, and RulesPanel translate DOM
  input into typed draft values and render coordinator state.
- Session adapters use the exact normalized boundary
  `{ sessionNumber: number, title: string, datePlayed: string, notes: string }`.
  Campaign ID, record ID, timestamps, linked entities, loading state, and row
  expansion are not draft content. `SessionRow` converts that value to
  `SessionInput`; a returned `Session` is normalized through the same boundary
  before acknowledgment. The row adapter also owns the transient confirmed-delete
  state: it disables fields and all Delete, Retry, and Discard actions while the
  delete command is pending, suppresses blur-triggered saves in that state, and
  re-enables the retained draft and recovery actions if deletion fails.
- Rule-note adapters use the exact normalized boundary `{ notes: string }`.
  Backend `null` and an empty textarea both normalize to `""`; the writer alone
  converts `""` back to `null` for IPC. Rule identity, collection, campaign,
  compiled body, category, page references, stale state, search, expansion, and
  objection text are not rule-note draft content. A returned `RuleEntry` is
  normalized through the same boundary before acknowledgment.
  Recovery presentation retains only `{ ruleId, title, collectionId }` beside
  the coordinator's note draft; it never caches a complete `RuleEntry`.
- Every adapter that awaits backend data owns a component-instance liveness
  guard. Cleanup invalidates that instance before any late completion can
  reconcile the app-lifetime coordinator or publish presentation. A replacement
  instance accepts only its own still-current request, even when both instances
  captured the same campaign and entity kind.
- After every structured promotion result, EntityManager derives its complete
  presentation from that result and the coordinator; it does not insert or
  select the older Create response as a competing display snapshot. For
  `source-promoted`, the returned `draft` supplies the row, preview, form, and
  status: canonical revision 1 is Saved, while a post-Create source revision
  remains visible as Unsaved. For `destination-converged`, the returned `draft`
  is the preserved destination, so its newer acknowledged name, preview, form
  content, and Saved status remain intact; the redundant source presentation
  disappears. For a blocked result, EntityManager reads the source and
  destination records separately and keeps each presentation distinct,
  including its own content and status. In no outcome may a generic "insert
  returned entity" path overwrite a coordinator-selected destination, collapse
  two retained drafts into one presentation, or perform persistence.
- `commands.ts` and Tauri command handlers remain the IPC adapters.
- `window-close.ts` adapts Tauri's close-request API behind a small port that is
  deterministic in component tests.
- `SaveStatus.svelte` and `CloseDraftsDialog.svelte` provide shared localized,
  accessible feedback without owning lifecycle policy.

No transient draft is added to SurrealDB or a Rust domain crate. The one backend
change is to make rule-note persistence provide a truthful acknowledgment.

## Editing Scope and Target Identities

| Draft kind      | Editing scope                                                 | Persistence target                         |
| --------------- | ------------------------------------------------------------- | ------------------------------------------ |
| Oracle          | `oracle:{campaign-id}` or `oracle:no-campaign`                | none                                       |
| Existing entity | `entity:{campaign-id}:{kind}:{record-id}`                     | `entity:{kind}:{record-id}`                |
| New entity      | `entity-new:{campaign-id}:{kind}:{client-draft-id}`           | the draft scope until Create returns an ID |
| Session         | `session:{campaign-id}:{session-id}`                          | `session:{session-id}`                     |
| Rule note       | `rule:{campaign-id-or-no-campaign}:{collection-id}:{rule-id}` | `rule:{rule-id}`                           |

The new-record client identity is allocated before backend creation and remains
stable until successful Create or explicit discard. The current product supports
one active new draft per campaign and entity kind; pressing New resumes it.
The coordinator is the source of truth for finding that draft; a component-local
selected-record flag or module map is not sufficient because the draft can be
hidden behind an existing record. An explicit create-from-link request queries
all `entity-new:{campaign-id}:{kind}:` scopes. If any retained draft is dirty,
saving, or failed, the existing focused confirmation offers exactly two
intentions: **Keep editing** reopens that retained draft unchanged, while
**Discard and create** discards only it and starts one stable new draft seeded
with the requested initial name. This is an explicit new-record action, not
ordinary navigation. A clean empty new draft may be reused and seeded without a
confirmation.

The client identity also remains the component's resume token while Create is
pending. If navigation unmounts the initiating EntityManager, another instance
can resume that client scope from the coordinator. When the first instance's
Create acknowledgment remaps the draft, the new instance reactively observes
`resolveScope(clientScope)`, switches its active scope and record ID to the
returned destination, and keeps the current value and status. Until an
authoritative entity-list refresh contains the returned ID, EntityManager
derives an acknowledged provisional row from the destination draft. That row is
an existing saved-domain record, not an unavailable or unsaved-new row; it is
used only for selection/form presentation and is never inserted into the
backend `entities` result or wikilink map.

Rule notes include the active campaign—or the explicit `no-campaign` context—in
the editing scope because CampaignView permits collection browsing without an
active campaign and the requirement forbids draft text appearing after a
campaign switch. Their persistence lane uses the rule ID because a regular
collection and its notes may be shared by campaigns; that prevents overlapping
writes to the same aggregate.

Session scopes include campaign even though a session ID is globally unique, so
an obsolete list or retained draft cannot appear after a campaign switch. The
lane omits campaign because every presentation of that session addresses the
same backend aggregate. Session-list loads reconcile only the exact
`session:{campaign-id}:` prefix. Rule-list loads reconcile only the exact
`rule:{campaign-id-or-no-campaign}:{collection-id}:` prefix; switching either
campaign or collection selects a different draft set while the shared
`rule:{rule-id}` lane continues to serialize writes to a shared rule entry.

## Invariants

1. Every draft has exactly one explicit editing scope.
2. A draft never appears in, or writes through, another campaign or record.
3. A new-record draft has a stable client identity before a backend ID exists.
4. Navigation, campaign switching, collection collapsing, and record selection
   never discard a draft.
5. Content-dirty is `current value != saved baseline`. An ordinary edit that
   returns to a baseline already known before that edit clears its pending state
   without a write; it does not pretend that the edit revision was acknowledged.
6. Only acknowledgment for the current revision can clear that revision's
   unacknowledged marker and label it Saved.
7. An older acknowledgment may advance the saved baseline to its canonical
   result, but it never replaces the current value or clears a newer
   unacknowledged revision. This remains true when the canonical result happens
   to equal the newer value.
8. Only one write is active for a persistence target. Repeated queued requests
   from one editing scope coalesce to its newest requested revision at the end
   of the target queue. A request from another scope sharing that target remains
   ordered and is never silently replaced by another scope's coalescing.
9. Failure preserves current value, scope, target, revision, and retryability.
10. Retry writes the latest value in the same scope to the same target.
11. Every exposed `DraftRecord` is readonly and runtime-frozen as a complete
    container. Its baseline, current value, in-flight attempt, and attempt
    payload are recursively frozen. Every change replaces it through a named
    pure transition; external mutation and coordinator-local object spread
    cannot bypass scope, target, revision, or acknowledgment rules.
12. Reopening an existing scope with a different target is a programmer error:
    `open` rejects it and leaves the existing draft unchanged. An authoritative
    current-context load refreshes both baseline and value only when the scope is
    fully clean: no pending, queued, saving, failed, or unacknowledged state.
    Any at-risk scope retains its local snapshots.
13. Discard affects exactly one scope; **Discard and close** is the separately
    explicit all-drafts operation. A scope cannot be discarded while its backend
    write is active, and the coordinator cannot discard all drafts while any
    backend write is active. A queued write that has not started may be canceled.
    After an active write settles, only the remaining unsaved revision may be
    discarded, against the newly applicable saved baseline.
14. A local or restored draft with an unacknowledged revision is never labeled
    Saved merely because its value coincides with a baseline learned from an
    older acknowledgment. Returning by a later edit to an already-known baseline
    is the explicit no-write exception in invariant 5.
15. Unsent Oracle drafts are never submitted automatically.
16. An unavailable target cannot redirect a save to another record or be treated
    as success.
17. While a new entity Create is active, its Create action is disabled and no
    second Create can be queued. The user may continue editing the draft, but no
    destructive abandonment or replacement can claim to cancel the already
    running write: those actions remain disabled with a localized explanation
    until it settles.
18. A successful Create acknowledgment supplies the only backend ID used to
    promote that draft. Promotion preserves a newer edit, changes the editing
    scope and persistence target atomically to that ID, and makes the next
    explicit action an Update/Save. It never issues a second Create.
19. Retained new drafts are discovered by campaign/kind scope, independently of
    which saved entity row or form is selected. At most one retained new draft
    exists for a campaign/kind.
20. An at-risk existing-entity draft remains discoverable even when an
    authoritative list no longer contains its record. The UI may synthesize a
    clearly unavailable row from the immutable draft snapshot, but it must not
    insert that row into domain data or retarget the draft.
21. Closing an entity form does not hide its state: every retained existing or
    new draft has a row-level **Unsaved changes**, **Saving…**, or **Couldn't
    save** indicator in its campaign/kind list until it is saved or explicitly
    discarded.
22. Frontend required-field validation does not start IPC and leaves the draft
    pending. A validation error returned by the backend is a recoverable failed
    save with Retry for the same scope, target, and latest content.
23. A successful Create remap leaves a reactive one-way redirect from the
    stable client scope to the acknowledged record scope. Resolution changes UI
    identity only; persistence operations use the target already stored on the
    destination draft and never infer a target from a redirect.
24. A remounted entity editor that resumed a client scope adopts its resolved
    record scope after Create acknowledgment. Both a clean acknowledgment and an
    acknowledgment with a newer revision remain visible and selectable even
    when an earlier list load omitted the new record; the latter remains
    Unsaved and its next explicit write is Update.
25. Adapter validation/error presentation is scoped. A delayed failure for
    Mira cannot appear on Torvin, and changing `EntityForm`'s editing scope
    clears frontend-only required-field messages without clearing either
    record's coordinator draft or recoverable failure.
26. EntityManager accepts a list response only when both its captured
    campaign/kind remain current and the adapter instance that issued the request
    is still live and authoritative. An unmounted or superseded instance cannot
    reconcile coordinator state or publish presentation, even when a replacement
    manager uses the same campaign/kind. The app-lifetime coordinator makes this
    adapter liveness check mandatory; it is not a draft-state transition.
27. After rejecting an unaccepted list response, EntityManager reconciles every
    backend entity row from an accepted response into its scoped coordinator
    draft immediately when the response completes, before exposing the response
    as presentation rows. A fully clean scope adopts that baseline; an at-risk
    scope keeps its local snapshots. Opening a cached presentation row later
    consumes the coordinator record established by reconciliation and must not
    apply the row's older baseline again. `open` receives the row baseline only
    when that scope has never been reconciled. Consequently, a list snapshot
    reconciled while Save is pending cannot overwrite the save acknowledgment
    after the row is clicked; a later accepted list load may refresh the scope
    once it is clean.
28. Create promotion may absorb an already-open destination only when its scope
    is the returned backend scope, its target exactly matches the returned
    backend target, it is fully clean with no active or queued work, its value
    exactly matches the Create canonical value, and its provenance has no
    acknowledged authority newer than the Create attempt. The transition
    atomically removes that disposable projection, moves the acknowledged source
    draft, and installs its redirect. Dirty, unacknowledged, saving, failed,
    queued, or target-mismatched destinations are never absorbed.
29. Authority is not inferred from content equality. Every applicable save
    acknowledgment retains the generation captured when its attempt began, even
    after the draft becomes clean or accepts a later list refresh. A destination
    Update begun after Create therefore remains newer authority when it
    acknowledges, including when its canonical content equals the source value.
30. When a fully clean exact-target destination has divergent or newer authority
    and the acknowledged source has no pending post-Create revision, promotion
    converges onto the destination without replacing its content. When the source
    has a pending revision, both drafts remain and a visible recoverable conflict
    requires the GM to choose **Keep saved record** or **Keep my draft**. The
    former discards only the source revision and converges; the latter promotes
    the source as pending to that exact target. Neither resolution writes
    automatically or issues another Create. Other refused promotions retain the
    existing Retry-only behavior. Every process-lifetime promotion issue counts
    as at-risk work, keeps its source discoverable across navigation, and is
    cleared only by successful promotion, convergence, explicit conflict
    resolution, or explicit discard of that source.
31. Entity presentation after Create is a projection of the coordinator result,
    never of the Create response in isolation. `source-promoted` presents the
    promoted source draft and its canonical-or-pending status;
    `destination-converged` presents the preserved destination draft, including
    its newer acknowledged name, preview, form content, and Saved status, while
    removing the redundant source presentation. A blocked result preserves
    separate source and destination presentations with their own content and
    status. Promotion, convergence, and blocked presentation handling issue no
    implicit Create or Update.
32. Acknowledgment applicability is checked against the current in-flight scope,
    target, attempt ID, and revision before canonical content is inspected or
    snapshotted. A stale or mismatched completion, even with malformed content,
    returns the unchanged draft and creates no error. Malformed canonical content
    on the applicable completion becomes a recoverable failure that clears only
    that in-flight attempt, preserves all draft work, and remains retryable for
    the same scope and target.
33. A session or rule list response cannot replace authority established after
    that request began. The adapter snapshots each known scope's acknowledged
    attempt generation at request start and declines a clean refresh when the
    scope has a newer generation at settlement. The next accepted request may
    refresh it.
34. Session and rule autosave are triggered only by an ordinary blur that
    remains in the same editing scope, or by explicit Retry. Input changes the
    draft immediately but never starts a write by itself. Focus moving to another
    app view or editing scope is navigation: the adapter suppresses the resulting
    blur save and retains the draft as pending. Recovery controls and Discard also
    suppress focus-loss blur; Retry owns its explicit request, while Discard
    starts no IPC. Navigation neither discards nor retargets an already pending,
    queued, saving, or failed request.
35. Session canonical trimming and rule-note `null` normalization are learned
    only from their applicable returned records. An adapter never constructs a
    successful baseline from its request payload or from `void` completion.
36. A session or rule failure is rendered only in the exact scope that started
    it. Switching campaign, collection, or record can hide that editor but
    cannot move its error, Retry, value, or target to the newly selected scope.
37. Confirmed session deletion is mutually exclusive with session editing and
    recovery. While deletion is pending, fields and Delete, Retry, and Discard
    are unavailable, and blur or recovery interaction cannot start or queue a
    save. Delete failure restores those controls without changing the draft.
38. Successful session deletion removes only the deleted session's editing
    scope and cancels only its queued save intent. It cannot remove or rewrite an
    unsent Oracle question or another session, campaign, or record draft.
39. Keyboard Discard of a session suppresses its focus-loss autosave, restores
    the complete saved baseline without IPC, and hands focus to that row's stable
    session header.
40. A deletion confirmation captures the complete immutable target when opened.
    Navigation shortcuts are suppressed while any modal is active, and later
    selection changes cannot alter what confirmation deletes.
41. Entity and campaign deletion cannot race an active target write. Successful
    deletion removes only captured scopes; failure preserves every draft.
42. New-session creation captures its campaign before IPC. Settlement may update
    only that campaign's presentation; a campaign switch cannot register or show
    the created session in the new campaign.
43. Only clean, inactive, unreferenced scopes may be released. Dirty, queued,
    saving, failed, unavailable, or promotion-conflicted drafts are never
    evicted. Release contains no domain write and no user-visible discard.
44. A close or application-exit request that finds at-risk work is synchronously
    prevented before async UI work. macOS application Quit, Dock Quit, window
    close, and equivalent normal termination paths share one decision state.
45. Drafts remain intact until native exit authorization succeeds. A rejected
    `confirm_app_exit` keeps the app open, preserves all work, shows an actionable
    persistent failure, and permits Retry or Cancel; only successful process
    termination makes a coordinator-wide discard observable.
46. Production exit is initiated by the typed Rust `confirm_app_exit` command.
    The frontend receives neither `core:window:allow-close` nor
    `core:window:allow-destroy`; wildcard window permissions are absent.

## Draft and Save Lifecycle

### Transition table

| Current observable state                            | Event                                                                                 | State / behavior                                                                                                                                                                                                                                                                  |
| --------------------------------------------------- | ------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Saved/clean                                         | Edit to a different value                                                             | **Unsaved changes**; increment revision                                                                                                                                                                                                                                           |
| Unsaved                                             | Edit back to already-known baseline                                                   | Clear unacknowledged marker; Clean/Saved; no write                                                                                                                                                                                                                                |
| Unsaved                                             | Request save                                                                          | **Saving…**; capture scope, target, revision, value                                                                                                                                                                                                                               |
| Saving                                              | Edit again                                                                            | Current value/revision advances; attempt payload stays immutable; UI remains Saving with newer unsaved content                                                                                                                                                                    |
| Saving                                              | Another save request for the same scope                                               | Do not overlap; remove that scope's older queued request and append only its newest requested revision, preserving waiters and cross-scope request order                                                                                                                          |
| Saving                                              | Request from another scope with the same target                                       | Do not overlap and do not coalesce it away; retain it in the target queue so its own scope receives its own acknowledgment or failure                                                                                                                                             |
| Saving                                              | Current-revision acknowledgment                                                       | Baseline becomes canonical acknowledgment; **Saved**; clear failure                                                                                                                                                                                                               |
| Saving                                              | Older-revision acknowledgment, newer edit                                             | Preserve current value; baseline becomes acknowledged payload; preserve the newer unacknowledged revision and remain **Unsaved changes**, even if the values now match; otherwise begin queued latest save                                                                        |
| Saving                                              | Stale/wrong-scope completion, including malformed canonical content                   | Check applicability first and ignore the completion without inspecting its canonical payload; preserve that scope's current state                                                                                                                                                 |
| Saving                                              | Applicable completion with malformed canonical content                                | **Couldn't save**; clear only the applicable in-flight attempt, preserve draft and baseline, and expose Retry for the same scope/target                                                                                                                                           |
| Saving                                              | Rejection                                                                             | **Couldn't save**; preserve draft; expose Retry                                                                                                                                                                                                                                   |
| Failed                                              | Retry                                                                                 | Capture latest revision for same target; **Saving…**                                                                                                                                                                                                                              |
| Queued, not active                                  | Discard changes                                                                       | Cancel that scope's queued request; restore only that scope's baseline; clear its error                                                                                                                                                                                           |
| Active backend write                                | Discard changes                                                                       | Block the action and announce that saving must finish; do not claim to undo the write                                                                                                                                                                                             |
| Settled with newer edit                             | Discard changes                                                                       | Restore only that scope to the baseline established by the completed write; clear remaining error/queued request                                                                                                                                                                  |
| New entity Create active                            | Edit again                                                                            | Keep Create disabled; preserve the newer revision in the client scope; do not queue or start another Create                                                                                                                                                                       |
| New entity Create active                            | Cancel, discard, or replacement intent                                                | Keep the destructive action disabled and describe that creation must finish first; preserve the client draft and permit no second Create                                                                                                                                          |
| New entity Create active                            | Successful acknowledgment for current revision                                        | Atomically remap the draft to the returned existing-record ID and target; canonical response becomes baseline; show **Saved**; subsequent explicit save uses Update                                                                                                               |
| New entity Create active                            | Successful acknowledgment for older revision                                          | Atomically remap to the returned existing-record ID and target; canonical response becomes baseline; preserve the newer value as **Unsaved changes**; enable explicit Save, which uses Update, never Create                                                                       |
| Create acknowledged, matching projection            | List opened the exact returned target and canonical value without newer authority     | Atomically replace the untouched projection with the acknowledged source draft, preserve any newer source revision, install the redirect, and continue with Update/Save semantics                                                                                                 |
| Create acknowledged, destination newer or divergent | Source has no post-Create pending revision                                            | Preserve the fully clean destination; remove the redundant source and install its redirect; derive row, preview, selected form, and Saved status from that destination rather than the older Create response; perform no backend write                                            |
| Create acknowledged, destination newer or divergent | Source has a pending post-Create revision                                             | Preserve both drafts and their distinct presentations; show a persistent promotion conflict; disable another Create; require explicit **Keep saved record** or **Keep my draft** without automatic persistence                                                                    |
| Create acknowledged, destination unsafe             | Destination is at-risk or target-mismatched                                           | Preserve both drafts and their distinct presentations; show a persistent promotion failure; disable another Create; Retry reattempts only local promotion after the transient conflict is resolved                                                                                |
| Structured promotion result                         | EntityManager updates selection and rows                                              | For `source-promoted`, project the returned source draft; for `destination-converged`, project the returned destination draft; for blocked, project source and destination independently; never reinsert the Create response as presentation authority and never write implicitly |
| Resumed client scope, Create settles                | Reactive scope redirect appears                                                       | Resolve to the returned record scope; preserve current content/status; present the acknowledged entity even if the already-completed list load omitted it; clean stays **Saved**, newer revision stays **Unsaved changes**                                                        |
| Remapped entity draft                               | Targeted discard via source or destination                                            | Affect only the resolved destination draft and remove redirects to it; never retarget or discard an unrelated record                                                                                                                                                              |
| Hidden retained new entity                          | Competing create-from-link request                                                    | Offer **Keep editing** to reopen it unchanged or **Discard and create** to remove only it and seed the requested name                                                                                                                                                             |
| Existing entity absent from reload                  | Retained at-risk draft exists                                                         | Synthesize an unavailable list row from the immutable draft; preserve row status, reopen, Retry, and targeted Discard                                                                                                                                                             |
| Pending entity draft                                | Frontend required-field validation fails                                              | Keep **Unsaved changes**; show field error; issue no IPC                                                                                                                                                                                                                          |
| Saving entity draft                                 | Backend validation rejection                                                          | **Couldn't save**; preserve target and content; expose Retry for the same record                                                                                                                                                                                                  |
| Another entity is active                            | Earlier entity's delayed failure settles                                              | Record failure on the originating coordinator scope only; do not show its field error or failure on the active entity                                                                                                                                                             |
| Entity form changes editing scope                   | Form-local validation exists                                                          | Clear the old scope's frontend-only validation message; retain both scoped draft values and any coordinator failures                                                                                                                                                              |
| Entity list load completes                          | Captured campaign/kind is current and issuing adapter is live                         | Reconcile every backend row into its coordinator scope before publishing presentation rows; clean scopes refresh and at-risk scopes retain local snapshots                                                                                                                        |
| Entity list load completes                          | Issuing adapter was unmounted or superseded                                           | Ignore the response completely, even if campaign/kind still match; do not reconcile the app-lifetime coordinator or publish presentation                                                                                                                                          |
| Cached entity row                                   | Click after reconciliation/save acknowledgment                                        | Select the existing coordinator scope without applying the cached row baseline again; preserve the acknowledged canonical value                                                                                                                                                   |
| Clean reconciled entity scope                       | A later current-context list load completes                                           | Reconcile the later backend row and refresh baseline/value; no request-version infrastructure is introduced                                                                                                                                                                       |
| Session/rule field                                  | Input                                                                                 | Normalize the complete editor value and revise only its exact scope; show **Unsaved changes**; start no write until blur or Retry                                                                                                                                                 |
| Session/rule field                                  | Ordinary blur within the same editing scope                                           | Request the latest revision through the record-target lane; show **Saving…**; later navigation may unmount presentation but does not cancel or retarget the attempt                                                                                                               |
| Focused session/rule field                          | Focus moves to another app view or editing scope                                      | Treat the focus change as navigation; suppress blur-save IPC and retain the current revision as **Unsaved changes** in its original scope                                                                                                                                         |
| Focused session/rule field                          | Retry, Discard, or another recovery action is activated                               | Suppress the focus-loss blur save; Retry owns the one explicit save request, while Discard restores its applicable baseline without IPC                                                                                                                                           |
| Session/rule list request                           | Context, request generation, and component instance remain current                    | Reconcile returned records only after comparing each existing scope's current acknowledged generation with the request-start snapshot; derive displayed editable values from coordinator drafts                                                                                   |
| Session/rule list request                           | Context changed, adapter unmounted, a newer load started, or scope authority advanced | Ignore the obsolete response or skip only the newer-authority scope; never replace a retained value, failure, or newly acknowledged clean baseline                                                                                                                                |
| Session/rule save                                   | Applicable canonical record returns                                                   | Normalize the returned record and acknowledge that attempt; never construct the baseline from the request; a newer local revision remains **Unsaved changes**                                                                                                                     |
| Session/rule save                                   | Target is missing or write fails                                                      | Keep the exact scope and value as **Couldn't save** with target-preserving Retry and Discard; no other campaign, collection, session, or rule changes                                                                                                                             |
| Failed session draft                                | NOT_FOUND detail returns                                                              | Preserve the exact value and target; render localized unavailable-target recovery rather than raw backend detail; Retry still addresses the same session                                                                                                                          |
| Dirty session field                                 | Keyboard Discard moves focus                                                          | Suppress the resulting blur autosave; restore the complete saved baseline without IPC; focus the stable session header; leave every unrelated draft intact                                                                                                                        |
| Session write active                                | Delete requested                                                                      | Keep Delete unavailable and announce that the session save must finish; do not start deletion                                                                                                                                                                                     |
| Settled session                                     | Delete confirmed                                                                      | Enter deleting state; disable fields and Delete, Retry, and Discard; ignore blur/recovery save triggers while the backend delete is pending                                                                                                                                       |
| Session deletion pending                            | Delete fails                                                                          | Preserve the draft and recovery state; re-enable fields and actions; start no save implicitly                                                                                                                                                                                     |
| Session deletion pending                            | Delete succeeds                                                                       | Cancel only that scope's queued save intent; remove exactly that scope and its row; preserve all unrelated drafts                                                                                                                                                                 |
| At-risk work                                        | Native close request                                                                  | Prevent close; focus Cancel in **Unsaved changes** dialog                                                                                                                                                                                                                         |
| Close dialog, write active                          | Discard and close                                                                     | Keep close prevented; disable the destructive action and announce that saving must finish                                                                                                                                                                                         |
| Close dialog, write settles with risk               | Save acknowledgment/failure                                                           | Keep the original close request prevented; enable Discard and close for any remaining unsaved/failed draft                                                                                                                                                                        |
| Close dialog, all risk settles clean                | Save acknowledgment                                                                   | Keep the original close prevented; announce **Saving finished. It is safe to close.** and replace the destructive action with **Close**                                                                                                                                           |
| Close dialog                                        | Escape/Cancel                                                                         | Keep drafts; restore focus to opener                                                                                                                                                                                                                                              |
| Close dialog, no active write                       | Discard and close                                                                     | Invoke the typed `confirm_app_exit` authorization without clearing drafts first; successful process teardown releases the in-memory coordinator; start no save                                                                                                                    |

### Sequence diagram

```text
GM input        Editor adapter        DraftCoordinator        IPC/backend
   | edit r7          |                      |                     |
   |----------------->| revise(scope,r7)     |                     |
   | blur/save        |--------------------->| begin r7            |
   |                  |                      |-------------------->|
   | edit r8          | revise(scope,r8)     |       write r7      |
   |----------------->|--------------------->|                     |
   |                  |                      |<--------------------|
   |                  |                      | ack r7              |
   |                  |                      | baseline=r7          |
   |                  |<---------------------| current stays r8    |
   |                  | renders Unsaved      |                     |
```

The same rule applies if the canonical `r7` response equals the current `r8`
value: content-dirty becomes false, but `r8` remains unacknowledged and the UI
remains **Unsaved changes** until an `r8` acknowledgment, an explicit discard,
or a subsequent deliberate edit returns to that now-known baseline.

Adapter liveness gates list authority before the coordinator boundary:

```text
Manager A          Manager B          DraftCoordinator        IPC/backend
   | list request       |                    |                     |
   |------------------------------------------------------------->|
   | unmount/obsolete   |                    |                     |
   |------------------X |                    |                     |
   |                    | list + save Mira   |                     |
   |                    |------------------->| write newer Mira    |
   |                    |                    |-------------------->|
   |                    |                    |<--------------------|
   |                    |                    | ack newer / Saved   |
   |<-------------------------------------------------------------|
   | old list settles   |                    |                     |
   | reject: not live   |                    |                     |
   | (no open, no rows) |                    | keeps newer Mira    |
```

Manager A's campaign/kind may still equal Manager B's. Its failed liveness check,
not a content comparison or coordinator transition, prevents the obsolete list
response from becoming authority.

## Interaction Contract

### Shared status and recovery

All localized editors use the same visible vocabulary:

- **Unsaved changes** when an unacknowledged revision exists, including the
  coincident-equality case where content-dirty is false.
- **Saving…** during the active attempt.
- **Saved** when the value equals the saved baseline and no unacknowledged
  revision remains. A current-revision acknowledgment or a later edit returning
  to an already-known baseline can establish this; an older acknowledgment alone
  cannot.
- **Couldn't save** plus the retained error detail and keyboard-operable
  **Retry** after failure.
- **Created, but needs attention** when backend Create succeeded but local
  promotion is blocked. Transient risk exposes Retry; a clean authority conflict
  exposes **Keep saved record** and **Keep my draft**. None performs an implicit
  backend write.
- **Discard changes** where explicitly abandoning a draft is offered.

`SaveStatus` is adjacent to the affected controls. Pending/saving/saved changes
use `role="status"` and `aria-live="polite"`; failure uses `role="alert"`.
Retry is a real button. Starting Retry may announce Saving, and successful
acknowledgment clears the failure. A persistent inline failure is primary;
toasts may supplement it but cannot replace it.

Recovery controls have explicit focus handoff because a successful transition
can remove the focused button from the DOM. When ordinary-save Retry succeeds,
focus moves to the stable editor control associated with that status (the same
field when practical, otherwise the editor heading/status anchor). When Create
promotion Retry or an explicit promotion resolution succeeds, focus moves to the
promoted entity editor or its stable status anchor. If the retry fails or the
promotion issue remains, focus stays on the still-present Retry control; if the
adapter must rerender it, focus is restored to its replacement. Focus is never
left on `document.body` or moved into a different record.

Both explicit promotion-conflict choices use a deterministic successful
handoff. **Keep saved record** focuses a stable editor control for the retained
destination; **Keep my draft** focuses a stable editor control for the promoted
pending draft. The selected conflict action is not removed until its intended
same-record focus target is available.

While that scope has an active backend write, **Discard changes** is disabled
because the client cannot undo the write already in progress. Adjacent polite
status text says **Wait for saving to finish before discarding changes**. Once the write
settles, focus remains on the editor/status area and Discard becomes available
for any remaining unsaved or failed revision. A queued request that has not
started may be canceled and discarded immediately.

### Oracle

Composer text is read/written through its campaign-specific scope. Switching
views or campaigns swaps to the correct retained value. The no-campaign context
uses its own scope. Returning to Oracle does not call `chatSend`; only Enter
without Shift or the Send button submits. Nonempty composer text is at-risk work
for close purposes and is described as retained for this running session, never
as saved domain content.

### Entities

Explicit Save/Create remains explicit. Input updates the scoped draft
immediately. Record and view navigation retain it without a confirmation. A row
with a retained draft exposes a text status badge—**Unsaved changes**,
**Saving…**, or **Couldn't save**—and reopening it restores the draft. The badge
is programmatically associated with the row and is not conveyed by color alone.
A retained new draft appears as its own draft row, named from its current value
with a localized untitled fallback. New resumes the one stable new-record draft
for that campaign/kind.

Save is serialized and uses a captured target. The form stays open after
success, is reseeded from canonical returned data, and shows Saved. If the GM
edits while Save is in progress, that edit remains visible and unsaved after the
older acknowledgment.

Create has a stricter lane policy than Update: while Create revision 1 is active,
the Create button is disabled and duplicate Create intent is ignored rather than
queued. The fields remain editable. When the acknowledgment returns a backend
ID and canonical revision-1 content, the coordinator atomically remaps the
client scope to `entity:{campaign}:{kind}:{returned-id}` and target
`entity:{kind}:{returned-id}`. If no newer edit exists, the form is Saved. If
revision 2 was entered meanwhile, its value remains visible and Unsaved against
the canonical revision-1 baseline; the button is now **Save**, and activating it
calls `updateEntity(returned-id, ...)`. The assigned identity appears once through
the structured promotion result and its retained draft, not through unconditional
insertion of the Create response, so no second entity can be created from the same
draft and no chosen destination presentation is overwritten.

The remap remains observable across EntityManager instances. A manager that
resumed the client scope while Create was pending derives the resolved active
scope with `draftCoordinator.resolveScope(activeDraftScope)`. When the redirect
appears, it atomically adopts the returned record ID and destination draft for
all subsequent reads, edits, Retry, and Save. It does not call `open` again and
does not manufacture a target. If that manager's earlier `getEntities` response
did not contain the returned record, its presentation rows include one
acknowledged provisional row derived from the destination scope and draft value.
The row/form remains present for both the clean-r1 and newer-r2 cases; it is
replaced by the canonical backend row on a later load and never enters the
backend array or wikilink index.

The opposite list/Create ordering is also safe. The backend can commit Create,
allow a remounted manager's entity list to open the returned record scope, and
only then deliver the Create response to the initiating manager. If that
destination is fully clean, its target exactly matches the response, its value
exactly matches the Create canonical content, and it has no newer acknowledged
authority, `tryRemapAfterCreate` treats it as an untouched projection of the same
persisted revision and atomically replaces it with the acknowledged client
draft.

Clean is not itself proof that a destination is disposable. Each started write
gets a coordinator-local authority generation, and an applicable acknowledgment
retains that generation on its scope. Suppose Create A commits but its response
is delayed, a remounted editor lists the new ID, and Update B to that ID is then
acknowledged before Create A's response arrives. B remains the newer authority
even though it is now fully clean and even if its canonical value equals A's
value. A later list refresh cannot erase B's retained authority generation. If
the source has no pending post-Create edit, identity resolution converges onto B:
B stays intact, A is removed as redundant, and the client-scope redirect points
to B. EntityManager must also keep B as the presentation authority: B's newer
name, notes preview, form content, and Saved status cannot be replaced by A's
older Create response. If A also carries a pending revision, neither authority
is silently chosen. Both remain available as distinct presentations under a
persistent promotion conflict until the GM explicitly chooses **Keep saved
record** or **Keep my draft**; the chosen local transition performs no backend
write, and saving the retained source is a subsequent explicit Update.

This is not last-writer-wins merging. Any dirty, unacknowledged, saving, failed,
queued, or target-mismatched destination is preserved and blocks promotion.
EntityManager reads the coordinator's retained promotion issue, keeps the source
and returned identity available across remounts, prevents another Create, and
renders **Created, but needs attention**. A transient destination risk exposes
Retry that reattempts promotion without backend I/O; a clean authority conflict
exposes the two explicit resolution choices. An unexpected collision therefore
loses neither draft and cannot create a duplicate entity.

Backend entity-list authority is applied at load completion, not deferred until
a row click. A response is accepted only after both the captured campaign/kind
check and the issuing component-instance liveness check pass. EntityManager
invalidates its instance in cleanup before a late promise can settle. An old
manager therefore cannot call `draftCoordinator.open` or publish rows after it
unmounts, even when a newly mounted manager has identical campaign/kind props.
After those guards accept a response, EntityManager maps each backend row to its
editing scope and target and calls `draftCoordinator.open` immediately, before
assigning the presentation list.
The coordinator therefore refreshes clean scopes at that moment and refuses to
replace any pending, saving, failed, queued, or otherwise unacknowledged scope.
The stored list row is thereafter only a display/navigation snapshot.
`openEdit` selects the existing coordinator draft and calls `open` with the row
baseline only when the scope does not exist because it was never reconciled.
It never reapplies a cached row merely because an intervening acknowledgment
made the scope clean. This ordering handles a save held across
unmount/remount: an old backend row returned while the save is active is ignored
by the coordinator, the acknowledgment establishes the canonical baseline, and
clicking that old rendered row preserves it. A genuinely later list completion
can refresh the now-clean draft through the same reconciliation path. A distinct
late response from the obsolete manager cannot perform that refresh; only a
still-live accepted request can supply authority. This adds a component-instance
liveness guard, not a state-machine transition, backend version, list-generation
service, or dependency.

Frontend name-required validation remains field-local: it sends no IPC and the
draft remains Unsaved. `EntityForm` receives the explicit active editing scope;
when that prop changes it clears only its frontend `nameError`, preventing a
required-name message from leaking from Mira to Torvin. `NOT_FOUND`, backend
`VALIDATION`, and database failures all preserve the form as **Couldn't save**
with Retry. EntityManager tags any field-oriented adapter error with the captured
save scope and passes it to EntityForm only when that scope equals the resolved
active scope. Therefore a delayed Mira rejection cannot appear on Torvin, while
reopening Mira reads the failed coordinator draft and still exposes its retained
content and Retry (with field detail as a same-instance enhancement). Retry
always captures the latest content for the original scope/target; backend
validation is not demoted to a local pending error merely because its error code
is `VALIDATION`.

Dirty Cancel becomes an explicit Discard changes confirmation; clean Cancel
simply closes. The dialog focuses Cancel, traps focus, closes on Escape, and
restores focus to the initiating control. If the entity's backend write is
active, Cancel remains focused and the destructive action is disabled with the
wait-for-saving announcement. After settlement, Discard restores the latest
acknowledged saved record or removes only the intended new draft; unrelated
Oracle/entity drafts remain.

The same truthfulness rule applies while Create is active. Cancel, discard, and
create-from-link replacement may open their normal confirmation context, but no
destructive control is operable while the Create promise is unsettled. The UI
describes that creation must finish before the draft can be abandoned, keeps the
safe action focused, preserves the client scope, and starts no second Create.

Create-from-link first queries retained new scopes for the requested
campaign/kind, even when the visible form is a saved entity. **Keep editing**
reopens the retained new draft and restores its current text. **Discard and
create** explicitly deletes only that draft, allocates a fresh stable client
identity, and initializes the replacement with the link name.
The confirmation initially focuses the safe **Keep editing** action, never the
destructive action. Escape has the same effect as canceling, leaves the retained
draft unchanged, and restores focus to the Create control that invoked the
request. Choosing either action is keyboard-operable; focus moves into the
resulting retained or replacement form only after the explicit choice.

Entity deletion captures `{ campaignId, kind, entityId, scope }` as one
immutable target when confirmation opens. The confirm callback uses only that
capture, never the current kind or selection. Shell navigation shortcuts are
suppressed while any modal is open. Delete is unavailable while the exact
entity target is saving; after backend success `removeAfterDelete(scope)`
removes its retained draft so a late acknowledgment cannot resurrect the row
and no unavailable draft can strand close protection. Failure preserves the
captured draft and presentation.

### Sessions

Title, date, and notes remain blur-save fields. The normalized draft is the
complete `SessionInput` shape—session number plus the three editable strings—so
every attempt is a coherent aggregate update. Record identity, campaign,
timestamps, expansion, and linked entities remain outside the value. Input
immediately creates **Unsaved changes** but does not write. Blur requests a save
through `session:{session-id}`. Rapid field changes cannot overlap writes; a
request received during a write is coalesced to the latest requested revision.
The returned `Session`, including backend title trimming, is the only successful
canonical acknowledgment.

Blur-save is editing-scope aware. An ordinary focus change that stays within the
same session scope may request autosave. Focus moving to another app view,
campaign, or record scope is navigation, so the adapter suppresses the resulting
blur request and retains the focused edit as **Unsaved changes**. Recovery
controls suppress that incidental blur too: Retry owns its explicit save request,
while Discard restores the baseline without IPC.

`Shell` passes the app-lifetime coordinator through `SessionLogView` and
`SessionList` to every keyed `SessionRow`. The view reloads in a campaign-keyed
effect, with component liveness and a monotonically increasing local request
generation. It captures known session acknowledgment generations before the
load starts. Only the still-current request can reconcile rows into the exact
campaign prefix, and a row whose save authority advanced after the request
started retains that acknowledged baseline. Editable header and form values are
always projected from coordinator records rather than cached list rows.

Failure is persistent and inline in the same session row with its retained
title, date, and notes, keyboard-operable Retry, and targeted Discard. A
not-found response is mapped to localized unavailable-target recovery copy; raw
backend detail is not rendered. It never removes the draft or changes another
row, and Retry keeps the same session target. If a later campaign list omits
that failed scope, the view presents a recovery-only unavailable row from its
complete session draft; after targeted discard the absent clean row disappears.
Navigation, campaign switching, and an accepted stale load cannot inject another
campaign's value or error. Successful Retry returns focus to the title when that
session row is expanded; if the row is collapsed, it uses the stable session
header as the same-scope fallback. Repeated failure restores focus to that same
row's replacement Retry button.

Session Discard is an explicit, keyboard-operable action. Activating it while an
edited field has focus suppresses the blur autosave caused by focus movement,
restores the complete saved baseline without IPC, and moves focus to the stable
session header. It affects no other draft, including an unsent Oracle question.

Deleting a session is already an explicit destructive intent. While its write
is active, Delete remains unavailable with the wait-for-saving explanation so an
update cannot race behind deletion. With no active write, the existing
confirmation remains required. From confirmation until the delete command
settles, every field and the Delete, Retry, and Discard actions are disabled;
blur, Retry, and any other attempted interaction cannot start or queue a session
save behind deletion. A successful delete cancels only that scope's queued
request through `removeAfterDelete`, removes exactly that coordinator scope, and
removes the row. A delete failure leaves the draft and recovery state intact and
re-enables the fields and recovery actions.

The existing New Session button continues to create a default backend session
immediately; designing a separate new-session form is outside this draft-loss
scope. Its click handler captures `campaignId` before IPC. Settlement may update
presentation only when that campaign remains current and the issuing view is
live. If the GM switched campaigns, the created record remains solely in its
original campaign and appears on the next authoritative load there; it is never
registered, selected, or rendered under the campaign that is current when the
promise settles. Creation failure uses persistent local error feedback.

### Rule-entry table notes

Rule notes remain blur-save. Each normalized draft is `{ notes: string }`;
backend `null` is `""` while editing and an empty attempted value is converted
back to `null` only at IPC. Compiled body, record metadata, expansion, search,
and redo-objection state never affect note dirty state. Each note has a
campaign/collection/record editing scope and a `rule:{rule-id}` target lane.
Thus Campaign A and B never display each other's retained drafts while two
presentations of a shared rule record still cannot write concurrently.

`CampaignView` passes both active campaign and the app-lifetime coordinator to
`RulesPanel`. Loads are keyed by campaign plus collection and use the same
liveness, request-generation, and request-start acknowledgment snapshot rule as
sessions. Collapse, Books/Rules tab changes, collection changes, view
navigation, and campaign switches retain and isolate values. A returned list
row refreshes a clean draft only when no newer acknowledgment occurred after
that list request began. The textarea always reads the coordinator projection,
never `notesDraft` or a cached `RuleEntry` baseline.

Failure is inline beneath that exact textarea with target-preserving Retry and
Discard. Rejection is caught by the coordinator path, so no unhandled promise is
emitted. Retry success focuses the same rule textarea; repeated failure restores
focus to that rule's replacement Retry control. Not-found remains recoverable
and cannot mutate another entry. A missing at-risk rule on reload may be exposed
as a recovery-only unavailable row identified by its retained target ID; it is
presentation, never persisted rule content.

`update_rule_notes` changes from `Result<(), String>` to
`Result<RuleEntry, String>`. The service executes `UPDATE ... RETURN AFTER`,
parses exactly one row, and returns a not-found error for zero rows. The Tauri
command preserves that result and `commands.ts` exposes `Promise<RuleEntry>`.
The adapter normalizes the returned record through `{ notes: string }`; it never
acknowledges from `void` or by echoing the request. This remains inside the
existing Codex rule aggregate and generic SurrealDB connection boundary: no new
trait, repository, persistence model, schema, dependency, or Rust draft domain
is added.

### Deleted or unavailable targets

A not-found response is a failed save, not permission to close the editor or
retarget the payload. The draft remains, the message states that the record is
no longer available, Retry keeps the same target, and Discard changes is
available. No other entity/session/rule is changed. On a later list reload, the
entity adapter merges backend rows with `listByPrefix` results for its exact
`entity:{campaign}:{kind}:` prefix. If an at-risk draft's record ID is absent,
the adapter synthesizes a presentation-only unavailable row from the immutable
draft name/baseline and status. Selecting it reopens the retained value and
inline Retry; it does not call `open` with a replacement baseline. The row is
never inserted into the backend list or wikilink registry. After explicit
discard, a still-absent clean target is no longer synthesized and disappears
from the presentation list.

Campaign deletion applies the same lifecycle rule at a prefix boundary. If any
campaign-owned lane is active, Delete is disabled and explains that saving must
finish. Otherwise confirmation enumerates at-risk campaign scopes and makes
campaign deletion explicit permission to discard only those scopes. The backend
delete receives the campaign captured when confirmation opened. Success removes
the captured scopes and releases remaining clean projections; failure preserves
them all. No campaign switch can retarget that request, and no hidden campaign
draft survives as an inaccessible close blocker after successful deletion.

### Normal close and restart policy

Normal termination has two adapters feeding one Rust `ExitAuthorization` state.
`getCurrentWindow().onCloseRequested` handles window-manager close. On platforms
where Tauri emits it, the Rust run callback handles `RunEvent::ExitRequested`.
That public event is not emitted for macOS Cmd-Q or Dock Quit, so the design does
not mislabel it as complete macOS coverage: a macOS
`applicationShouldTerminate` adapter must return terminate-later, publish the
same exit intent, and complete only after the shared authorization decision.
Concurrent window/application requests coalesce into one intent/nonce and never
double-exit. The frontend window adapter synchronously calls `preventDefault()`
for at-risk work. Registration failure is fail-closed as already designed:
editable UI is withheld behind localized Retry.

With no at-risk draft, the frontend invokes `confirm_app_exit(intent, "keep")`;
the Rust adapter validates the pending intent and permits exactly one native
application exit. With a nonempty Oracle draft or any dirty,
saving, queued, or failed editor draft, the request remains prevented and the
shared modal opens:

- Title: **Unsaved changes**
- Primary safe action, initially focused: **Cancel**
- Destructive action when work remains and no write is active: **Discard and close**
- Safe completion action if an existing write settles all work: **Close**

Escape is Cancel. Cancel restores focus to the element that had focus before the
native request. When any backend write is already active, **Discard and close**
is disabled, Cancel stays initially focused, and a polite live message says
**Wait for saving to finish before discarding and closing.** The close request
remains prevented; Chronacle neither cancels nor starts a backend write. If the
write settles with remaining unsaved or failed work, the destructive action is
enabled and the dialog retains focus. If every at-risk draft becomes clean, the
dialog instead announces **Saving finished. It is safe to close.** and replaces
the destructive action with **Close**. Focus remains on Cancel unless the GM
moves it; neither settlement path steals focus. The original close remains
prevented until the GM activates an enabled close action. This avoids both
claiming to undo an in-progress write and auto-closing on an asynchronous
result.

With no active backend write, **Discard and close** begins a two-phase operation.
It first captures the coordinator's at-risk snapshot and invokes the Rust
`confirm_app_exit(intent, "discard")` command; it does not mutate drafts
beforehand. The Rust command validates the pending intent and requests native
application termination. Only successful termination permits process teardown
to release memory. If authorization or native termination setup rejects, the app
remains open, every draft is byte-for-byte intact, and the dialog shows a
persistent localized failure with keyboard-operable Retry and Cancel. Retry
uses the same decision against the still-pending intent; Cancel revokes it and
restores the original opener. Neither path calls a writer or chat submission.
This treats process teardown—not pre-emptive `discardAll()`—as the commit point
for global discard.

Because production close is initiated by a Rust command, the frontend needs
neither `core:window:allow-close` nor `core:window:allow-destroy`; both grants and
all wildcard window permissions are removed. The macOS adapter must use public
APIs already available through approved dependencies and forward rather than
replace Tauri's application delegate behavior. If an implementation spike shows
that this cannot be done maintainably without a new crate, unsafe private API,
or delegate replacement, the macOS portion is a concrete architecture blocker:
stop for an ADR/dependency decision rather than ship an incomplete Quit claim.

An OS kill, power loss, crash, or forced process termination cannot be
intercepted; drafts are then lost. Durable storage is deliberately excluded to
avoid retaining private campaign text in plaintext localStorage or adding an
encrypted storage design to this focused change. The user guide states this
limit. Saved SurrealDB content remains durable as before.

## Approach Comparison

| Approach                                         | Benefits                                                                                                                            | Costs / risks                                                                                                                                      | Decision                |
| ------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- |
| Keep every view mounted and add local flags      | Small apparent code change                                                                                                          | Hidden views still run effects/listeners; campaign and record isolation remains scattered; races and close behavior remain unsolved                | Rejected                |
| Persist every draft in localStorage or SurrealDB | Crash/restart recovery                                                                                                              | Private campaign text at rest, versioning/cleanup/migration/security policy, stale-record recovery UX, and a new backend concept                   | Rejected for this scope |
| App-scoped in-memory coordinator + close guard   | One lifecycle policy, precise scope/revision rules, no draft-state dependency or private plaintext persistence, deterministic tests | Drafts do not survive crashes; normal close needs native integration; ADR-013 adds two approved target-specific dependencies for the macOS adapter | Chosen                  |

The chosen approach fits Chronacle's Tauri IPC architecture: transient editing
rules remain frontend-local, Rust continues to own persisted aggregates, and IPC
success remains the authoritative boundary between a draft and saved content.

For Create/list promotion, three narrower rules were considered:

| Promotion rule                                   | Benefit                                              | Failure mode / cost                                                                                                    |
| ------------------------------------------------ | ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| Absorb any fully clean exact-target destination  | No additional metadata                               | Unsafe: a newer acknowledged Update becomes clean and can be overwritten by the older Create acknowledgment            |
| Permanently block any destination ever saved     | Safe and simple                                      | Overly conservative; cannot converge redundant settled sources and does not express which write established authority  |
| Retain save-start authority generation per scope | Precise ordering without backend/schema dependencies | One small coordinator-only provenance map and explicit conflict handling when both source and destination contain work |

The generation rule is chosen. Save-start order, rather than response-arrival
order or wall-clock time, identifies Update B as newer than already-running
Create A even when A's delayed response arrives last. Exact content comparison
still permits an untouched list projection of A's canonical record to be
absorbed. Content equality never substitutes for revision acknowledgment, so a
same-content newer Update remains authoritative and a pending source revision
remains pending.

For entity discovery, a component/module registry was rejected because it can
fall out of sync across unmounts and cannot reliably expose a failed draft after
a backend reload removes its row. Mutating `entities` with fake `GraphNode`
records was also rejected because presentation-only unavailable content could
leak into wikilinks or domain actions. The chosen `listByPrefix` coordinator
query keeps one app-lifetime source of truth; EntityManager derives retained-new
and synthesized unavailable presentation rows without persisting them.

## Acceptance Criteria

The six flat executable Gherkin files are the canonical observable contract:

- [Oracle drafts](../../../apps/desktop/tests/e2e/features/draft-save-reliability-oracle.feature)
- [Entity drafts](../../../apps/desktop/tests/e2e/features/draft-save-reliability-entities.feature)
- [Session drafts](../../../apps/desktop/tests/e2e/features/draft-save-reliability-sessions.feature)
- [Rule-note drafts](../../../apps/desktop/tests/e2e/features/draft-save-reliability-rules.feature)
- [Campaign deletion](../../../apps/desktop/tests/e2e/features/draft-save-reliability-campaigns.feature)
- [Normal closing](../../../apps/desktop/tests/e2e/features/draft-save-reliability-close.feature)

Their matching flat step modules mirror those editor and lifecycle boundaries.
This design retains the lifecycle rationale and scenario inventory, while the
feature files remain the single maintained source for executable scenario bodies.

The established acceptance inventory remains: campaign-scoped Oracle retention
without submission; existing/new entity retention and promotion; explicit
discard; revision-aware serialized entity/session/rule saves; unavailable-target
recovery; accessible status, retry and focus; and no implicit save during
navigation or close. Pure state/component tests cover eligible clean-scope
release and prove at-risk scopes are never evicted. Native tests cover the
window/application exit boundary against an isolated temporary application-data
directory. Rust command integration covers real rule-note result/error
serialization; the frontend adapter test covers wrong-target rejection.

## Testing Strategy

- Pure Vitest tests exercise identity, the normalized value boundary, cyclic and
  unsupported-value rejection, runtime immutability of every complete draft
  record and nested attempt/value exposed by each transition and coordinator
  query, content-dirty versus unacknowledged revision, revision monotonicity,
  stale acknowledgments (including canonical content equal to a newer edit),
  applicability-before-canonical-validation for malformed stale completions,
  applicable malformed canonical recovery, retry, discard blocking
  during active writes, target lanes, clean authoritative refresh, mismatched
  target rejection, prefix isolation, immutable draft listing, create promotion
  with a newer revision, exact-value untouched-projection absorption, retained
  save-start authority across clean refresh, same-content and divergent newer
  destination acknowledgment, source-to-destination convergence, explicit
  promotion-conflict resolution, refusal to absorb any at-risk or
  target-mismatched destination, invalid promotion rejection, coalescing, and
  target-queue coalescing that replaces only the same scope while preserving
  distinct scopes sharing a target, waiter settlement, chronological final
  intent, and deferred promises without timers.
- Svelte component tests verify restored values, status announcements, inline
  failure, backend-validation Retry versus frontend required-field validation,
  create-r1/edit-r2 remapping without duplicate creation, cross-instance scope
  resolution after navigation during Create, provisional representation when a
  prior list load omitted the returned ID, Create/list interleaving where the
  exact-target destination is listed before acknowledgment, a destination Update
  acknowledged after Create starts but before Create responds, visible retryable
  promotion failure or explicit conflict choices without another Create,
  no-source-r2 automatic convergence whose row, preview, selected form, and Saved
  state retain newer destination content with exactly one Create and one Update,
  blocked promotion with distinct source and destination presentations, deferred update across
  unmount/remount with an old list row reconciled before acknowledgment and
  clicked afterward, a later clean-list refresh, rejection of an obsolete
  same-context manager's list response after its replacement acknowledges newer
  content, delayed failure isolation
  between Mira and Torvin, frontend-validation reset on record change,
  hidden-new-draft keep/replace with safe initial focus/Escape/opener restoration,
  held-Create abandonment with disabled/described destructive actions and no
  second Create, stable same-record focus after both explicit promotion-conflict
  choices,
  row-level retained status, unavailable-row reload/reopen, keyboard Retry with
  successful focus handoff and repeated-failure focus retention, disabled discard
  while saving, post-settlement recovery, discard confirmation, and focus
  restoration. Deferred promises,
  rather than clocks or timers, control the list and update completion order.
- Session adapter tests use complete normalized values to prove title trimming,
  date/notes retention, baseline reversion without IPC, per-session coalescing,
  equal-content stale acknowledgment, campaign isolation, recovery-only missing
  rows, scope-aware blur suppression during navigation and recovery actions, and
  the request-start authority snapshot. They explicitly resolve a
  pre-acknowledgment list response after the save acknowledgment, then prove a
  genuinely later load can refresh the clean scope. Retry focus assertions cover
  expanded-title success, collapsed-header fallback, and repeated-failure Retry
  retention.
- Rule-note adapter tests normalize `null`/empty consistently and cover
  campaign-plus-collection isolation, collapse/remount retention, target-lane
  serialization across two campaign scopes for the same shared rule, stale
  completion, no unhandled rejection, missing-target Retry/Discard, and
  successful/repeated-failure focus handoff for the exact textarea.
- Coordinator tests prove `release` removes clean inactive scopes, refuses every
  at-risk state, cleans redirects only with their released target, and never
  treats release as discard. Rule-panel tests inspect retained presentation and
  prove complete rule bodies and references are not cached.
- Entity/Campaign component tests use deferred writes and deletes to prove
  immutable confirmation targets, modal shortcut suppression, active-write
  exclusion, exact-scope cleanup, and preservation on delete failure.
- Session integration tests cover a Create started in campaign A and settled
  after switching to B, plus the complete failure → NOT_FOUND list omission →
  navigation → recovery-only row → same-target Retry/Discard journey.
- Rule-note coverage has two boundaries: a frontend adapter test rejects a
  canonical acknowledgment carrying another rule ID, and a Rust/Tauri command
  integration test invokes the real command adapter to assert the serialized
  canonical record and typed NOT_FOUND error shape.
- Executable Gherkin drives the real frontend through the established IPC mock.
  The mock gains deterministic deferred responses and an in-memory persisted
  record model, so assertions cover visible values and persistence outcomes, not
  merely command invocation.
- No extra Gherkin scenario is added for container freezing or validation order:
  those are non-visual state-boundary rules. The existing observable failed-save
  and Retry scenarios continue to cover the user-facing result of an applicable
  malformed acknowledgment, while focused state/coordinator tests establish the
  ordering and immutability guarantees directly.
- Rust service tests make rule-note update return the updated record—including
  the canonical `notes` value—and reject a missing ID; the Tauri command smoke
  test and frontend wrapper typecheck verify the return type reaches the adapter.
- A Linux `tauri-driver` check exercises two real window-manager Alt+F4 journeys:
  an Oracle close decision and closing while a session save is already running.
  It uses a temporary application data/config/cache root and a fresh database;
  it never reads or mutates developer state. Both journeys execute without a
  skip. Portable application-Quit authorization, rejection/Retry, and concurrent
  request behavior are covered by frontend component, state-machine, Rust unit,
  and static adapter tests rather than Linux native UI. The macOS AppKit adapter
  has no runtime verification in this Linux environment.
- Final verification is `scripts/ci/local-pr.sh`, plus the focused native close
  check after a RocksDB Tauri build. Evidence and both independent reviews must
  refer to the same final `git rev-parse HEAD` plus dirty diff state.

## Unrelated Findings

- Session entity-load failures are console-only, and campaign/collection/list
  loaders have inconsistent visible failure treatment.
- Oracle chat-history loads and some CampaignView subscription loads lack stale
  response guards. They should be addressed in a separate data-loading race
  pass; the draft coordinator prevents these reads from overwriting drafts.
- EntityManager uses a separate transient toast instead of the app-wide toast
  store. Save failures move inline in this change; consolidating all remaining
  entity notifications is separate cleanup.
