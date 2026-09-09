import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import {
  acknowledge,
  beginAttempt,
  createDraft,
  discard as discardDraft,
  draftValuesEqual,
  isContentDirty,
  refreshCleanDraft,
  rejectAttempt,
  remapDraftIdentity,
  revise as reviseDraft,
  type DraftAttempt,
  type DraftRecord,
  type DraftSnapshot,
  type DraftValue,
} from './draft-state';

type SaveWriter<T extends DraftValue> = (value: DraftSnapshot<T>) => Promise<T>;
type DiscardResult = 'discarded' | 'blocked-active-save' | 'missing';
type DiscardAllResult = Exclude<DiscardResult, 'missing'>;

export type CreatePromotionBlockReason =
  | 'missing-source'
  | 'source-not-settled'
  | 'destination-target-mismatch'
  | 'destination-at-risk'
  | 'destination-newer-acknowledgment'
  | 'destination-conflict';

export type CreatePromotionResolution = 'source-promoted' | 'destination-converged';

export type CreatePromotionResult<T extends DraftValue> =
  | {
      readonly outcome: 'promoted';
      readonly resolution: CreatePromotionResolution;
      readonly draft: DraftRecord<T>;
    }
  | { readonly outcome: 'blocked'; readonly reason: CreatePromotionBlockReason };

export interface CreatePromotionIssue {
  readonly sourceScope: string;
  readonly destinationScope: string;
  readonly destinationTarget: string;
  readonly reason: CreatePromotionBlockReason;
}

interface SaveWaiter {
  resolve(): void;
}

interface SaveRequest {
  scope: string;
  target: string;
  revision: number;
  writer: SaveWriter<DraftValue>;
  waiters: SaveWaiter[];
}

interface SaveLane {
  active: boolean;
  queued: SaveRequest | null;
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === 'string') return error;
  if (
    typeof error === 'object' &&
    error !== null &&
    'message' in error &&
    typeof error.message === 'string'
  ) {
    return error.message;
  }
  try {
    return JSON.stringify(error) ?? String(error);
  } catch {
    return String(error);
  }
}

export class DraftCoordinator {
  private readonly drafts = new SvelteMap<string, DraftRecord<DraftValue>>();
  private readonly lanes = new SvelteMap<string, SaveLane>();
  private readonly redirects = new SvelteMap<string, string>();
  private readonly createPromotionIssues = new SvelteMap<string, CreatePromotionIssue>();
  private readonly authoritativeListScopes = new SvelteSet<string>();
  private nextAttemptId = 1;

  open<T extends DraftValue>(
    scope: string,
    target: string | null,
    baseline: T,
    origin: 'editor' | 'authoritative-list' = 'editor',
  ): DraftRecord<T> {
    const existing = this.drafts.get(scope);
    if (existing && existing.target !== target) {
      throw new Error(
        `Draft scope ${scope} is already associated with target ${String(existing.target)}`,
      );
    }

    if (existing && !this.isFullyClean(existing)) return existing as DraftRecord<T>;

    const authoritative = existing
      ? refreshCleanDraft(existing as DraftRecord<T>, baseline)
      : createDraft(scope, target, baseline);

    this.drafts.set(scope, authoritative as DraftRecord<DraftValue>);
    if (origin === 'authoritative-list') this.authoritativeListScopes.add(scope);
    return authoritative;
  }

  get<T extends DraftValue>(scope: string): DraftRecord<T> | undefined {
    return this.drafts.get(scope) as DraftRecord<T> | undefined;
  }

  revise<T extends DraftValue>(scope: string, value: T): DraftRecord<T> {
    const current = this.get<T>(scope);
    if (!current) throw new Error(`Draft scope is not open: ${scope}`);

    const revised = reviseDraft(current, value);
    if (revised !== current) this.drafts.set(scope, revised as DraftRecord<DraftValue>);
    return revised;
  }

  async requestSave<T extends DraftValue>(scope: string, writer: SaveWriter<T>): Promise<void> {
    const draft = this.get<T>(scope);
    if (!draft) throw new Error(`Draft scope is not open: ${scope}`);
    if (draft.target === null) throw new Error('Draft has no persistence target');
    if (!this.needsSave(draft) && draft.error === null) return;

    return new Promise<void>((resolve) => {
      const request: SaveRequest = {
        scope,
        target: draft.target as string,
        revision: draft.revision,
        writer: writer as unknown as SaveWriter<DraftValue>,
        waiters: [{ resolve }],
      };
      const lane = this.lanes.get(request.target) ?? { active: false, queued: null };
      this.lanes.set(request.target, lane);

      if (lane.active) {
        request.waiters.unshift(...(lane.queued?.waiters ?? []));
        lane.queued = request;
        return;
      }

      lane.active = true;
      void this.run(request, lane);
    });
  }

  canDiscard(scope: string): boolean {
    const resolvedScope = this.resolveScope(scope);
    const draft = this.drafts.get(resolvedScope);
    return draft !== undefined && draft.inFlight === null;
  }

  discard(scope: string): DiscardResult {
    const resolvedScope = this.resolveScope(scope);
    const draft = this.drafts.get(resolvedScope);
    if (!draft) return 'missing';
    if (!this.canDiscard(resolvedScope)) return 'blocked-active-save';

    this.cancelQueued(resolvedScope);
    if (resolvedScope.startsWith('entity-new:')) this.drafts.delete(resolvedScope);
    else this.drafts.set(resolvedScope, discardDraft(draft));
    this.createPromotionIssues.delete(scope);
    this.createPromotionIssues.delete(resolvedScope);
    this.removeRedirectsFor(resolvedScope);
    return 'discarded';
  }

  hasActiveWrites(): boolean {
    for (const lane of this.lanes.values()) {
      if (lane.active) return true;
    }
    return false;
  }

  canDiscardAll(): boolean {
    return !this.hasActiveWrites();
  }

  discardAll(): DiscardAllResult {
    if (!this.canDiscardAll()) return 'blocked-active-save';

    for (const lane of this.lanes.values()) {
      if (lane.queued) this.resolve(lane.queued);
    }
    this.lanes.clear();
    this.drafts.clear();
    this.redirects.clear();
    this.createPromotionIssues.clear();
    this.authoritativeListScopes.clear();
    return 'discarded';
  }

  atRiskCount(): number {
    const atRiskScopes: string[] = [];
    for (const draft of this.drafts.values()) {
      if (this.needsSave(draft) || draft.inFlight !== null || draft.error !== null) {
        atRiskScopes.push(draft.scope);
      }
    }
    for (const issue of this.createPromotionIssues.values()) {
      if (!atRiskScopes.includes(issue.sourceScope)) atRiskScopes.push(issue.sourceScope);
    }
    return atRiskScopes.length;
  }

  listByPrefix<T extends DraftValue>(prefix: string): ReadonlyArray<DraftRecord<T>> {
    const matches: Array<DraftRecord<T>> = [];
    for (const draft of this.drafts.values()) {
      if (!draft.scope.startsWith(prefix)) continue;
      matches.push(draft as DraftRecord<T>);
    }
    return Object.freeze(matches);
  }

  resolveScope(scope: string): string {
    return this.redirects.get(scope) ?? scope;
  }

  remapAfterCreate<T extends DraftValue>(
    sourceScope: string,
    destinationScope: string,
    destinationTarget: string,
  ): DraftRecord<T> {
    const result = this.tryRemapAfterCreate<T>(sourceScope, destinationScope, destinationTarget);
    if (result.outcome === 'blocked') {
      throw new Error(this.createPromotionError(result.reason, sourceScope, destinationScope));
    }
    return result.draft;
  }

  tryRemapAfterCreate<T extends DraftValue>(
    sourceScope: string,
    destinationScope: string,
    destinationTarget: string,
  ): CreatePromotionResult<T> {
    if (
      this.redirects.has(sourceScope) ||
      this.redirects.has(destinationScope) ||
      this.hasRedirectTo(sourceScope) ||
      this.hasRedirectTo(destinationScope)
    ) {
      throw new Error('Create draft remap cannot use a redirected source or destination');
    }
    const source = this.drafts.get(sourceScope);
    if (!source) {
      return this.blockCreatePromotion(
        sourceScope,
        destinationScope,
        destinationTarget,
        'missing-source',
      );
    }
    if (
      source.inFlight !== null ||
      this.hasQueued(sourceScope) ||
      source.error !== null ||
      source.acknowledgedRevision === 0
    ) {
      return this.blockCreatePromotion(
        sourceScope,
        destinationScope,
        destinationTarget,
        'source-not-settled',
      );
    }

    const destination = this.drafts.get(destinationScope);
    if (destination && destination.target !== destinationTarget) {
      return this.blockCreatePromotion(
        sourceScope,
        destinationScope,
        destinationTarget,
        'destination-target-mismatch',
      );
    }
    if (destination && !this.isFullyClean(destination)) {
      return this.blockCreatePromotion(
        sourceScope,
        destinationScope,
        destinationTarget,
        'destination-at-risk',
      );
    }
    for (const draft of this.drafts.values()) {
      if (
        draft.scope !== sourceScope &&
        draft.scope !== destinationScope &&
        draft.target === destinationTarget
      ) {
        return this.blockCreatePromotion(
          sourceScope,
          destinationScope,
          destinationTarget,
          'destination-at-risk',
        );
      }
    }
    if (this.lanes.has(destinationTarget)) {
      return this.blockCreatePromotion(
        sourceScope,
        destinationScope,
        destinationTarget,
        'destination-at-risk',
      );
    }

    if (destination) {
      const exactProjection =
        draftValuesEqual(destination.value, source.baseline) &&
        (this.authoritativeListScopes.has(destinationScope) ||
          destination.lastAcknowledgedAttemptId === 0) &&
        destination.lastAcknowledgedAttemptId <= source.lastAcknowledgedAttemptId;
      if (!exactProjection) {
        if (!this.needsSave(source)) {
          return this.convergeOnDestination<T>(sourceScope, destinationScope, destination);
        }
        const reason: CreatePromotionBlockReason =
          destination.lastAcknowledgedAttemptId > source.lastAcknowledgedAttemptId
            ? 'destination-newer-acknowledgment'
            : 'destination-conflict';
        return this.blockCreatePromotion(sourceScope, destinationScope, destinationTarget, reason);
      }
    }

    return this.promoteSource<T>(sourceScope, destinationScope, destinationTarget, source);
  }

  getCreatePromotionIssue(sourceScope: string): CreatePromotionIssue | undefined {
    const issue = this.createPromotionIssues.get(sourceScope);
    return issue ? Object.freeze({ ...issue }) : undefined;
  }

  retryCreatePromotion<T extends DraftValue>(sourceScope: string): CreatePromotionResult<T> {
    const issue = this.createPromotionIssues.get(sourceScope);
    if (!issue) return { outcome: 'blocked', reason: 'missing-source' };
    return this.tryRemapAfterCreate<T>(
      issue.sourceScope,
      issue.destinationScope,
      issue.destinationTarget,
    );
  }

  resolveCreatePromotion<T extends DraftValue>(
    sourceScope: string,
    choice: 'keep-destination' | 'keep-source',
  ): CreatePromotionResult<T> {
    const issue = this.createPromotionIssues.get(sourceScope);
    if (
      !issue ||
      (issue.reason !== 'destination-conflict' &&
        issue.reason !== 'destination-newer-acknowledgment')
    ) {
      return { outcome: 'blocked', reason: 'missing-source' };
    }

    const source = this.drafts.get(sourceScope);
    if (
      !source ||
      source.inFlight !== null ||
      this.hasQueued(sourceScope) ||
      source.error !== null ||
      source.acknowledgedRevision === 0
    ) {
      return this.blockCreatePromotion(
        issue.sourceScope,
        issue.destinationScope,
        issue.destinationTarget,
        'source-not-settled',
      );
    }

    const destination = this.drafts.get(issue.destinationScope);
    if (!destination || destination.target !== issue.destinationTarget) {
      return this.blockCreatePromotion(
        issue.sourceScope,
        issue.destinationScope,
        issue.destinationTarget,
        'destination-target-mismatch',
      );
    }
    if (!this.isFullyClean(destination) || this.lanes.has(issue.destinationTarget)) {
      return this.blockCreatePromotion(
        issue.sourceScope,
        issue.destinationScope,
        issue.destinationTarget,
        'destination-at-risk',
      );
    }
    for (const draft of this.drafts.values()) {
      if (
        draft.scope !== sourceScope &&
        draft.scope !== issue.destinationScope &&
        draft.target === issue.destinationTarget
      ) {
        return this.blockCreatePromotion(
          issue.sourceScope,
          issue.destinationScope,
          issue.destinationTarget,
          'destination-at-risk',
        );
      }
    }

    if (choice === 'keep-destination') {
      return this.convergeOnDestination<T>(sourceScope, issue.destinationScope, destination);
    }
    return this.promoteSource<T>(
      sourceScope,
      issue.destinationScope,
      issue.destinationTarget,
      source,
    );
  }

  private async run(request: SaveRequest, lane: SaveLane): Promise<void> {
    const current = this.drafts.get(request.scope);
    if (
      !current ||
      current.target !== request.target ||
      current.revision !== request.revision ||
      (!this.needsSave(current) && current.error === null)
    ) {
      this.finish(request, lane);
      return;
    }

    const { draft, attempt } = beginAttempt(current, this.nextAttemptId++);
    this.drafts.set(request.scope, draft);

    try {
      const canonical = await request.writer(attempt.value);
      this.applyAcknowledgment(attempt, canonical);
    } catch (error) {
      const latest = this.drafts.get(attempt.scope);
      if (latest) {
        this.drafts.set(attempt.scope, rejectAttempt(latest, attempt, errorMessage(error)));
      }
    }

    this.finish(request, lane);
  }

  private applyAcknowledgment(attempt: DraftAttempt<DraftValue>, canonical: DraftValue): void {
    const latest = this.drafts.get(attempt.scope);
    if (!latest) return;
    this.drafts.set(attempt.scope, acknowledge(latest, attempt, canonical));
  }

  private finish(request: SaveRequest, lane: SaveLane): void {
    this.resolve(request);
    lane.active = false;
    const queued = lane.queued;
    lane.queued = null;

    if (queued) {
      lane.active = true;
      void this.run(queued, lane);
    } else {
      this.lanes.delete(request.target);
    }
  }

  private resolve(request: SaveRequest): void {
    for (const waiter of request.waiters) waiter.resolve();
  }

  private cancelQueued(scope: string): void {
    for (const lane of this.lanes.values()) {
      if (lane.queued?.scope === scope) {
        this.resolve(lane.queued);
        lane.queued = null;
      }
    }
  }

  private hasQueued(scope: string): boolean {
    for (const lane of this.lanes.values()) {
      if (lane.queued?.scope === scope) return true;
    }
    return false;
  }

  private hasRedirectTo(scope: string): boolean {
    for (const destination of this.redirects.values()) {
      if (destination === scope) return true;
    }
    return false;
  }

  private removeRedirectsFor(scope: string): void {
    this.redirects.delete(scope);
    for (const [source, destination] of this.redirects) {
      if (destination === scope) this.redirects.delete(source);
    }
  }

  private promoteSource<T extends DraftValue>(
    sourceScope: string,
    destinationScope: string,
    destinationTarget: string,
    source: DraftRecord<DraftValue>,
  ): CreatePromotionResult<T> {
    const remapped = remapDraftIdentity(
      source as DraftRecord<T>,
      destinationScope,
      destinationTarget,
    );
    this.drafts.delete(sourceScope);
    this.drafts.set(destinationScope, remapped as DraftRecord<DraftValue>);
    this.authoritativeListScopes.delete(sourceScope);
    this.authoritativeListScopes.add(destinationScope);
    this.redirects.set(sourceScope, destinationScope);
    this.createPromotionIssues.delete(sourceScope);
    return { outcome: 'promoted', resolution: 'source-promoted', draft: remapped };
  }

  private convergeOnDestination<T extends DraftValue>(
    sourceScope: string,
    destinationScope: string,
    destination: DraftRecord<DraftValue>,
  ): CreatePromotionResult<T> {
    this.drafts.delete(sourceScope);
    this.authoritativeListScopes.delete(sourceScope);
    this.redirects.set(sourceScope, destinationScope);
    this.createPromotionIssues.delete(sourceScope);
    return {
      outcome: 'promoted',
      resolution: 'destination-converged',
      draft: destination as DraftRecord<T>,
    };
  }

  private blockCreatePromotion<T extends DraftValue>(
    sourceScope: string,
    destinationScope: string,
    destinationTarget: string,
    reason: CreatePromotionBlockReason,
  ): CreatePromotionResult<T> {
    this.createPromotionIssues.set(sourceScope, {
      sourceScope,
      destinationScope,
      destinationTarget,
      reason,
    });
    return { outcome: 'blocked', reason };
  }

  private createPromotionError(
    reason: CreatePromotionBlockReason,
    sourceScope: string,
    destinationScope: string,
  ): string {
    if (reason === 'missing-source') return `Create draft is missing or not open: ${sourceScope}`;
    if (reason === 'source-not-settled') {
      return 'Create draft must be acknowledged and settled before remapping';
    }
    if (reason === 'destination-target-mismatch') {
      return `Destination scope has a mismatched target: ${destinationScope}`;
    }
    if (reason === 'destination-newer-acknowledgment') {
      return `Destination has a newer acknowledged save: ${destinationScope}`;
    }
    if (reason === 'destination-conflict') {
      return `Destination conflicts with the created draft: ${destinationScope}`;
    }
    return `Destination is occupied or at risk: ${destinationScope}`;
  }

  private isFullyClean(draft: DraftRecord<DraftValue>): boolean {
    return (
      !this.needsSave(draft) &&
      draft.inFlight === null &&
      draft.error === null &&
      !this.hasQueued(draft.scope)
    );
  }

  private needsSave<T extends DraftValue>(draft: DraftRecord<T>): boolean {
    return draft.unacknowledgedRevision !== null || isContentDirty(draft);
  }
}
