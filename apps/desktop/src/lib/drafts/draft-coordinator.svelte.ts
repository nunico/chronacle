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
type DeleteCleanupResult = 'removed' | 'blocked-active-save' | 'missing';
type ReleaseResult = 'released' | 'retained-at-risk' | 'missing';
type PrefixDeleteCleanupResult = number | 'blocked-active-save';

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
  queued: SaveRequest[];
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
  private readonly leaseCounts = new SvelteMap<string, number>();
  private readonly deferredReleases = new SvelteSet<string>();
  private nextAttemptId = 1;
  private closeDecisionActive = false;
  private navigationTransitionCount = 0;

  beginCloseDecision(): void {
    this.closeDecisionActive = true;
  }

  endCloseDecision(): void {
    this.closeDecisionActive = false;
  }

  isCloseDecisionActive(): boolean {
    return this.closeDecisionActive;
  }

  beginNavigationTransition(): () => void {
    this.navigationTransitionCount += 1;
    let active = true;
    return () => {
      if (!active) return;
      active = false;
      this.navigationTransitionCount = Math.max(0, this.navigationTransitionCount - 1);
    };
  }

  isNavigationTransitionActive(): boolean {
    return this.navigationTransitionCount > 0;
  }

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
      const lane = this.lanes.get(request.target) ?? { active: false, queued: [] };
      this.lanes.set(request.target, lane);

      if (lane.active) {
        const previousIndex = lane.queued.findIndex(({ scope: queuedScope }) => {
          return queuedScope === request.scope;
        });
        if (previousIndex !== -1) {
          const [previous] = lane.queued.splice(previousIndex, 1);
          if (previous) request.waiters.unshift(...previous.waiters);
        }
        lane.queued.push(request);
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

  acquireLease(scope: string): () => void {
    const resolvedScope = this.resolveScope(scope);
    if (!this.drafts.has(resolvedScope)) {
      throw new Error(`Cannot lease a draft scope that is not open: ${scope}`);
    }
    this.leaseCounts.set(resolvedScope, (this.leaseCounts.get(resolvedScope) ?? 0) + 1);
    let active = true;
    return () => {
      if (!active) return;
      active = false;
      const currentScope = this.resolveScope(scope);
      const count = this.leaseCounts.get(currentScope) ?? 0;
      if (count <= 1) this.leaseCounts.delete(currentScope);
      else this.leaseCounts.set(currentScope, count - 1);
      this.maybeReleaseDeferred(currentScope);
    };
  }

  discard(scope: string): DiscardResult {
    const resolvedScope = this.resolveScope(scope);
    const draft = this.drafts.get(resolvedScope);
    if (!draft) return 'missing';
    if (!this.canDiscard(resolvedScope)) return 'blocked-active-save';

    this.cancelQueued(resolvedScope);
    if (resolvedScope.startsWith('entity-new:') || resolvedScope.startsWith('session-new:')) {
      this.drafts.delete(resolvedScope);
    } else this.drafts.set(resolvedScope, discardDraft(draft));
    this.createPromotionIssues.delete(scope);
    this.createPromotionIssues.delete(resolvedScope);
    this.removeRedirectsFor(resolvedScope);
    this.maybeReleaseDeferred(resolvedScope);
    return 'discarded';
  }

  removeAfterDelete(scope: string): DeleteCleanupResult {
    const resolvedScope = this.resolveScope(scope);
    const draft = this.drafts.get(resolvedScope);
    if (!draft) return 'missing';
    if (draft.inFlight !== null) return 'blocked-active-save';

    this.cancelQueued(resolvedScope);
    this.drafts.delete(resolvedScope);
    this.authoritativeListScopes.delete(resolvedScope);
    this.leaseCounts.delete(resolvedScope);
    this.deferredReleases.delete(resolvedScope);
    this.createPromotionIssues.delete(scope);
    this.createPromotionIssues.delete(resolvedScope);
    this.removeRedirectsFor(resolvedScope);
    return 'removed';
  }

  release(scope: string): ReleaseResult {
    const resolvedScope = this.resolveScope(scope);
    const draft = this.drafts.get(resolvedScope);
    if (!draft) return 'missing';
    if (!this.isReleaseEligible(draft) || (this.leaseCounts.get(resolvedScope) ?? 0) > 0) {
      this.deferredReleases.add(resolvedScope);
      return 'retained-at-risk';
    }

    this.removeReleasedScope(resolvedScope);
    return 'released';
  }

  releasePrefix(prefix: string): number {
    this.requirePrefix(prefix);
    let released = 0;
    for (const scope of [...this.drafts.keys()]) {
      if (this.matchesAnyPrefix(scope, [prefix]) && this.release(scope) === 'released')
        released += 1;
    }
    return released;
  }

  hasActiveWritesByPrefixes(prefixes: readonly string[]): boolean {
    this.requirePrefixes(prefixes);
    for (const draft of this.drafts.values()) {
      if (draft.inFlight !== null && this.matchesAnyPrefix(draft.scope, prefixes)) return true;
    }
    return false;
  }

  removeAfterDeletePrefixes(prefixes: readonly string[]): PrefixDeleteCleanupResult {
    this.requirePrefixes(prefixes);
    if (this.hasActiveWritesByPrefixes(prefixes)) return 'blocked-active-save';

    const matchingScopes = [...this.drafts.keys()].filter((scope) => {
      return this.matchesAnyPrefix(scope, prefixes);
    });
    for (const scope of matchingScopes) this.cancelQueued(scope);
    for (const scope of matchingScopes) {
      this.drafts.delete(scope);
      this.authoritativeListScopes.delete(scope);
      this.leaseCounts.delete(scope);
      this.deferredReleases.delete(scope);
    }
    for (const [source, destination] of this.redirects) {
      if (this.matchesAnyPrefix(source, prefixes) || this.matchesAnyPrefix(destination, prefixes)) {
        this.redirects.delete(source);
      }
    }
    for (const [source, issue] of this.createPromotionIssues) {
      if (
        this.matchesAnyPrefix(source, prefixes) ||
        this.matchesAnyPrefix(issue.destinationScope, prefixes)
      ) {
        this.createPromotionIssues.delete(source);
      }
    }
    return matchingScopes.length;
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
      for (const queued of lane.queued) this.resolve(queued);
    }
    this.lanes.clear();
    this.drafts.clear();
    this.redirects.clear();
    this.createPromotionIssues.clear();
    this.authoritativeListScopes.clear();
    this.leaseCounts.clear();
    this.deferredReleases.clear();
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
    const queued = lane.queued.shift();

    if (queued) {
      lane.active = true;
      void this.run(queued, lane);
    } else {
      this.lanes.delete(request.target);
    }
    this.maybeReleaseDeferred(request.scope);
  }

  private resolve(request: SaveRequest): void {
    for (const waiter of request.waiters) waiter.resolve();
  }

  private cancelQueued(scope: string): void {
    for (const lane of this.lanes.values()) {
      const retained: SaveRequest[] = [];
      for (const queued of lane.queued) {
        if (queued.scope === scope) this.resolve(queued);
        else retained.push(queued);
      }
      lane.queued = retained;
    }
  }

  private hasQueued(scope: string): boolean {
    for (const lane of this.lanes.values()) {
      if (lane.queued.some(({ scope: queuedScope }) => queuedScope === scope)) return true;
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
    this.moveLeaseState(sourceScope, destinationScope);
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
    this.moveLeaseState(sourceScope, destinationScope);
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

  private isReleaseEligible(draft: DraftRecord<DraftValue>): boolean {
    if (!this.isFullyClean(draft)) return false;
    for (const issue of this.createPromotionIssues.values()) {
      if (issue.sourceScope === draft.scope || issue.destinationScope === draft.scope) return false;
    }
    return true;
  }

  private maybeReleaseDeferred(scope: string): void {
    const resolvedScope = this.resolveScope(scope);
    if (!this.deferredReleases.has(resolvedScope)) return;
    const draft = this.drafts.get(resolvedScope);
    if (!draft) {
      this.deferredReleases.delete(resolvedScope);
      return;
    }
    if ((this.leaseCounts.get(resolvedScope) ?? 0) > 0 || !this.isReleaseEligible(draft)) return;
    this.removeReleasedScope(resolvedScope);
  }

  private removeReleasedScope(scope: string): void {
    this.drafts.delete(scope);
    this.authoritativeListScopes.delete(scope);
    this.leaseCounts.delete(scope);
    this.deferredReleases.delete(scope);
    this.removeRedirectsFor(scope);
  }

  private moveLeaseState(sourceScope: string, destinationScope: string): void {
    const sourceLeases = this.leaseCounts.get(sourceScope) ?? 0;
    if (sourceLeases > 0) {
      this.leaseCounts.set(
        destinationScope,
        (this.leaseCounts.get(destinationScope) ?? 0) + sourceLeases,
      );
    }
    this.leaseCounts.delete(sourceScope);
    if (this.deferredReleases.delete(sourceScope)) this.deferredReleases.add(destinationScope);
  }

  private matchesAnyPrefix(scope: string, prefixes: readonly string[]): boolean {
    return prefixes.some((prefix) => {
      return scope === prefix || (prefix.endsWith(':') && scope.startsWith(prefix));
    });
  }

  private requirePrefix(prefix: string): void {
    if (prefix.length === 0) throw new Error('Draft scope prefix cannot be empty');
  }

  private requirePrefixes(prefixes: readonly string[]): void {
    if (prefixes.length === 0) throw new Error('At least one draft scope prefix is required');
    for (const prefix of prefixes) this.requirePrefix(prefix);
  }

  private needsSave<T extends DraftValue>(draft: DraftRecord<T>): boolean {
    return draft.unacknowledgedRevision !== null || isContentDirty(draft);
  }
}
