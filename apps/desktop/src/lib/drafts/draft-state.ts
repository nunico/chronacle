export type DraftScalar = null | boolean | number | string;

export type DraftValue =
  | DraftScalar
  | readonly DraftValue[]
  | { readonly [key: string]: DraftValue };

export type DraftSnapshot<T extends DraftValue> = T extends DraftScalar
  ? T
  : T extends ReadonlyArray<infer Item extends DraftValue>
    ? ReadonlyArray<DraftSnapshot<Item>>
    : {
        readonly [Key in keyof T]: T[Key] extends DraftValue ? DraftSnapshot<T[Key]> : never;
      };

export type DraftStatus = 'saved' | 'pending' | 'saving' | 'failed';

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

export function oracleScope(campaignId: string | null): string {
  return `oracle:${campaignId ?? 'no-campaign'}`;
}

export function entityScope(campaignId: string, kind: string, recordId: string): string {
  return `entity:${campaignId}:${kind}:${recordId}`;
}

export function newEntityScope(campaignId: string, kind: string, clientDraftId: string): string {
  return `entity-new:${campaignId}:${kind}:${clientDraftId}`;
}

export function sessionScope(campaignId: string, sessionId: string): string {
  return `session:${campaignId}:${sessionId}`;
}

export function newSessionScope(campaignId: string, clientDraftId: string): string {
  return `session-new:${campaignId}:${clientDraftId}`;
}

export function ruleScope(campaignId: string | null, collectionId: string, ruleId: string): string {
  return `rule:${campaignId ?? 'no-campaign'}:${collectionId}:${ruleId}`;
}

function invalidValue(reason: string): never {
  throw new TypeError(`Draft value must be normalized acyclic JSON-like data: ${reason}`);
}

function snapshotValue(value: unknown, ancestors: Set<object>): DraftValue {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return value;
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) invalidValue('numbers must be finite');
    return value;
  }
  if (typeof value !== 'object') invalidValue(`unsupported ${typeof value} value`);

  if (ancestors.has(value)) invalidValue('cycles are not supported');
  ancestors.add(value);

  try {
    if (Array.isArray(value)) {
      const keys = Object.keys(value);
      if (keys.length !== value.length || Reflect.ownKeys(value).length !== value.length + 1) {
        invalidValue('arrays must be dense and unadorned');
      }

      const snapshot: DraftValue[] = [];
      for (let index = 0; index < value.length; index += 1) {
        const descriptor = Object.getOwnPropertyDescriptor(value, index);
        if (!descriptor?.enumerable || !('value' in descriptor)) {
          invalidValue('array items must be dense data properties');
        }
        snapshot.push(snapshotValue(descriptor.value, ancestors));
      }
      return Object.freeze(snapshot);
    }

    const prototype = Object.getPrototypeOf(value) as object | null;
    if (prototype !== Object.prototype && prototype !== null) {
      invalidValue('objects must be plain');
    }

    const entries: Array<[string, DraftValue]> = [];
    for (const key of Reflect.ownKeys(value)) {
      if (typeof key !== 'string') invalidValue('symbol keys are not supported');
      const descriptor = Object.getOwnPropertyDescriptor(value, key);
      if (!descriptor?.enumerable || !('value' in descriptor)) {
        invalidValue('object properties must be enumerable data properties');
      }
      entries.push([key, snapshotValue(descriptor.value, ancestors)]);
    }
    return Object.freeze(Object.fromEntries(entries) as Record<string, DraftValue>);
  } finally {
    ancestors.delete(value);
  }
}

function snapshot<T extends DraftValue>(value: T): DraftSnapshot<T> {
  return snapshotValue(value, new Set()) as DraftSnapshot<T>;
}

function freezeRecord<T extends DraftValue>(record: DraftRecord<T>): DraftRecord<T> {
  return Object.freeze(record);
}

export function draftValuesEqual(left: DraftValue, right: DraftValue): boolean {
  if (Object.is(left, right)) return true;
  if (left === null || right === null || typeof left !== 'object' || typeof right !== 'object') {
    return false;
  }
  if (Array.isArray(left) || Array.isArray(right)) {
    if (!Array.isArray(left) || !Array.isArray(right) || left.length !== right.length) return false;
    return left.every((item, index) => draftValuesEqual(item, right[index]));
  }

  const leftRecord = left as Readonly<Record<string, DraftValue>>;
  const rightRecord = right as Readonly<Record<string, DraftValue>>;
  const leftKeys = Object.keys(leftRecord).sort();
  const rightKeys = Object.keys(rightRecord).sort();
  if (leftKeys.length !== rightKeys.length) return false;
  return leftKeys.every(
    (key, index) =>
      key === rightKeys[index] &&
      draftValuesEqual(leftRecord[key] as DraftValue, rightRecord[key] as DraftValue),
  );
}

export function createDraft<T extends DraftValue>(
  scope: string,
  target: string | null,
  baseline: T,
): DraftRecord<T> {
  const saved = snapshot(baseline);
  return freezeRecord({
    scope,
    target,
    baseline: saved,
    value: saved,
    revision: 0,
    acknowledgedRevision: 0,
    lastAcknowledgedAttemptId: 0,
    unacknowledgedRevision: null,
    inFlight: null,
    error: null,
  });
}

export function revise<T extends DraftValue>(draft: DraftRecord<T>, value: T): DraftRecord<T> {
  const nextValue = snapshot(value);
  if (draftValuesEqual(draft.value, nextValue)) return draft;

  const revision = draft.revision + 1;
  const returnedToBaseline = draftValuesEqual(nextValue, draft.baseline);
  return freezeRecord({
    ...draft,
    value: nextValue,
    revision,
    unacknowledgedRevision: returnedToBaseline ? null : revision,
    error: returnedToBaseline ? null : draft.error,
  });
}

export function beginAttempt<T extends DraftValue>(
  draft: DraftRecord<T>,
  attemptId: number,
): { draft: DraftRecord<T>; attempt: DraftAttempt<T> } {
  if (draft.target === null) throw new Error('Draft has no persistence target');
  const attempt: DraftAttempt<T> = Object.freeze({
    id: attemptId,
    scope: draft.scope,
    target: draft.target,
    revision: draft.revision,
    value: snapshot(draft.value as T),
  });
  return { attempt, draft: freezeRecord({ ...draft, inFlight: attempt, error: null }) };
}

function isApplicable<T extends DraftValue>(
  draft: DraftRecord<T>,
  attempt: DraftAttempt<T>,
): boolean {
  const active = draft.inFlight;
  return (
    active !== null &&
    active.id === attempt.id &&
    active.scope === attempt.scope &&
    active.scope === draft.scope &&
    active.target === attempt.target &&
    active.target === draft.target &&
    active.revision === attempt.revision
  );
}

export function acknowledge<T extends DraftValue>(
  draft: DraftRecord<T>,
  attempt: DraftAttempt<T>,
  canonicalValue: T,
): DraftRecord<T> {
  if (!isApplicable(draft, attempt)) return draft;
  const baseline = snapshot(canonicalValue);

  return freezeRecord({
    ...draft,
    baseline,
    value: draft.revision === attempt.revision ? baseline : draft.value,
    acknowledgedRevision: Math.max(draft.acknowledgedRevision, attempt.revision),
    lastAcknowledgedAttemptId: Math.max(draft.lastAcknowledgedAttemptId, attempt.id),
    unacknowledgedRevision:
      draft.revision === attempt.revision ? null : draft.unacknowledgedRevision,
    inFlight: null,
    error: null,
  });
}

export function rejectAttempt<T extends DraftValue>(
  draft: DraftRecord<T>,
  attempt: DraftAttempt<T>,
  error: string,
): DraftRecord<T> {
  if (!isApplicable(draft, attempt)) return draft;
  return freezeRecord({ ...draft, inFlight: null, error });
}

export function discard<T extends DraftValue>(draft: DraftRecord<T>): DraftRecord<T> {
  return freezeRecord({
    ...draft,
    value: draft.baseline,
    unacknowledgedRevision: null,
    inFlight: null,
    error: null,
  });
}

export function refreshCleanDraft<T extends DraftValue>(
  draft: DraftRecord<T>,
  baseline: T,
): DraftRecord<T> {
  const saved = snapshot(baseline);
  return freezeRecord({
    scope: draft.scope,
    target: draft.target,
    baseline: saved,
    value: saved,
    revision: 0,
    acknowledgedRevision: 0,
    lastAcknowledgedAttemptId: draft.lastAcknowledgedAttemptId,
    unacknowledgedRevision: null,
    inFlight: null,
    error: null,
  });
}

export function remapDraftIdentity<T extends DraftValue>(
  draft: DraftRecord<T>,
  scope: string,
  target: string,
): DraftRecord<T> {
  return freezeRecord({ ...draft, scope, target });
}

export function isContentDirty<T extends DraftValue>(draft: DraftRecord<T>): boolean {
  return !draftValuesEqual(draft.value, draft.baseline);
}

export function statusOf<T extends DraftValue>(draft: DraftRecord<T>): DraftStatus {
  if (draft.inFlight !== null) return 'saving';
  if (draft.error !== null) return 'failed';
  return draft.unacknowledgedRevision !== null || isContentDirty(draft) ? 'pending' : 'saved';
}
