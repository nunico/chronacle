import { describe, expect, it, vi } from 'vitest';
import { DraftCoordinator } from './draft-coordinator.svelte';
import {
  entityScope,
  newEntityScope,
  oracleScope,
  statusOf,
  type DraftRecord,
  type DraftValue,
} from './draft-state';

interface Deferred<T> {
  promise: Promise<T>;
  resolve(value: T): void;
  reject(error: unknown): void;
}

function deferred<T>(): Deferred<T> {
  let resolvePromise!: (value: T) => void;
  let rejectPromise!: (error: unknown) => void;
  const promise = new Promise<T>((resolve, reject) => {
    resolvePromise = resolve;
    rejectPromise = reject;
  });

  return { promise, resolve: resolvePromise, reject: rejectPromise };
}

function required<T>(value: T | undefined, description: string): T {
  if (value === undefined) throw new Error(`Expected ${description}`);
  return value;
}

class ControlledWriter<T> {
  readonly attempts: Array<{ value: T; completion: Deferred<T> }> = [];
  readonly persisted: T[] = [];
  activeWrites = 0;
  maxConcurrentWrites = 0;

  readonly write = (value: T): Promise<T> => {
    const completion = deferred<T>();
    this.attempts.push({ value, completion });
    this.activeWrites += 1;
    this.maxConcurrentWrites = Math.max(this.maxConcurrentWrites, this.activeWrites);

    return completion.promise
      .then((canonical) => {
        this.persisted.push(canonical);
        return canonical;
      })
      .finally(() => {
        this.activeWrites -= 1;
      });
  };
}

describe('DraftCoordinator retention and scope', () => {
  it('freezes every record shell exposed while coordinating the draft lifecycle', async () => {
    const coordinator = new DraftCoordinator();
    const saveCompletion = deferred<{ notes: string }>();
    const failingCompletion = deferred<{ notes: string }>();
    const scope = entityScope('camp-a', 'npc', 'mira');
    const failingScope = entityScope('camp-a', 'npc', 'torvin');

    const opened = coordinator.open(scope, 'entity:npc:mira', { notes: 'saved' });
    const exposed = required(coordinator.get<{ notes: string }>(scope), 'the opened draft');
    const revised = coordinator.revise(scope, { notes: 'changed' });
    const save = coordinator.requestSave(scope, () => saveCompletion.promise);
    const saving = required(coordinator.get<{ notes: string }>(scope), 'the saving draft');
    saveCompletion.resolve({ notes: 'canonical changed' });
    await save;
    const acknowledged = required(
      coordinator.get<{ notes: string }>(scope),
      'the acknowledged draft',
    );
    coordinator.discard(scope);
    const discarded = required(coordinator.get<{ notes: string }>(scope), 'the discarded draft');

    coordinator.open(failingScope, 'entity:npc:torvin', { notes: 'saved' });
    coordinator.revise(failingScope, { notes: 'failed value' });
    const failedSave = coordinator.requestSave(failingScope, () => failingCompletion.promise);
    failingCompletion.reject(new Error('offline'));
    await failedSave;
    const rejected = required(
      coordinator.get<{ notes: string }>(failingScope),
      'the rejected draft',
    );

    expect(opened).toBe(exposed);
    expect(revised).not.toBe(opened);
    expect(saving).not.toBe(revised);
    expect(acknowledged).not.toBe(saving);
    expect(discarded).not.toBe(acknowledged);

    const records = [
      ['open/get', opened],
      ['revise', revised],
      ['start save', saving],
      ['acknowledge', acknowledged],
      ['reject save', rejected],
      ['discard', discarded],
    ] as const;
    for (const [transition, record] of records) {
      expect.soft(Object.isFrozen(record), `${transition} record shell`).toBe(true);
      const original = {
        target: record.target,
        revision: record.revision,
        acknowledgedRevision: record.acknowledgedRevision,
        inFlight: record.inFlight,
        error: record.error,
      };
      expect
        .soft(() =>
          Object.assign(record as unknown as Record<string, unknown>, {
            target: 'entity:npc:wrong-target',
            revision: 999,
            acknowledgedRevision: 999,
            inFlight: null,
            error: 'mutated through an exposed record',
          }),
        )
        .toThrow(TypeError);
      expect.soft(record, `${transition} lifecycle metadata`).toMatchObject(original);
    }

    expect(coordinator.get(scope)).toBe(discarded);
    expect(coordinator.get(scope)).toMatchObject({
      target: 'entity:npc:mira',
      revision: 1,
      acknowledgedRevision: 1,
      inFlight: null,
      error: null,
    });
    expect(coordinator.get(failingScope)).toBe(rejected);
    expect(coordinator.get(failingScope)).toMatchObject({
      target: 'entity:npc:torvin',
      revision: 1,
      acknowledgedRevision: 0,
      inFlight: null,
      error: 'offline',
    });
  });

  it('recursively snapshots and freezes values exposed by open and get', () => {
    const coordinator = new DraftCoordinator();
    const scope = entityScope('camp-a', 'npc', 'mira');
    const baselineInput = { notes: 'saved', metadata: { tags: ['seer'] } };

    const opened = coordinator.open(scope, 'entity:npc:mira', baselineInput);
    baselineInput.metadata.tags.push('mutated through open input');

    expect(opened.value).toEqual({ notes: 'saved', metadata: { tags: ['seer'] } });
    expect(Object.isFrozen(opened.value.metadata.tags)).toBe(true);

    const exposed = required(coordinator.get<typeof baselineInput>(scope), 'the opened draft');
    expect(() => {
      (exposed.value.metadata.tags as string[]).push('mutated through get');
    }).toThrow(TypeError);

    expect(coordinator.get<typeof baselineInput>(scope)).toMatchObject({
      baseline: { notes: 'saved', metadata: { tags: ['seer'] } },
      value: { notes: 'saved', metadata: { tags: ['seer'] } },
      revision: 0,
      unacknowledgedRevision: null,
    });
    expect(statusOf(required(coordinator.get(scope), 'the unchanged draft'))).toBe('saved');
  });

  it('retains a draft in the app-lifetime map when no component is subscribed', async () => {
    const coordinator = new DraftCoordinator();
    const scope = entityScope('camp-a', 'npc', 'mira');
    coordinator.open(scope, 'entity:npc:mira', { notes: 'saved' });
    coordinator.revise(scope, { notes: 'retained while away' });

    await Promise.resolve();

    expect(coordinator.get<{ notes: string }>(scope)).toMatchObject({
      scope,
      target: 'entity:npc:mira',
      baseline: { notes: 'saved' },
      value: { notes: 'retained while away' },
    });
    expect(
      coordinator.open(scope, 'entity:npc:mira', { notes: 'later server value' }).value,
    ).toEqual({ notes: 'retained while away' });
  });

  it('does not expose one campaign or record draft through another scope', () => {
    const coordinator = new DraftCoordinator();
    const mira = entityScope('camp-a', 'npc', 'mira');
    const torvin = entityScope('camp-a', 'npc', 'torvin');
    const otherCampaign = entityScope('camp-b', 'npc', 'mira');

    coordinator.open(mira, 'entity:npc:mira', { notes: 'Mira saved' });
    coordinator.open(torvin, 'entity:npc:torvin', { notes: 'Torvin saved' });
    coordinator.open(otherCampaign, 'entity:npc:mira', { notes: 'Other campaign saved' });
    coordinator.revise(mira, { notes: 'Mira private draft' });

    expect(coordinator.get<{ notes: string }>(torvin)?.value.notes).toBe('Torvin saved');
    expect(coordinator.get<{ notes: string }>(otherCampaign)?.value.notes).toBe(
      'Other campaign saved',
    );
  });

  it('represents Oracle as local-only and refuses to attach a save writer', async () => {
    const coordinator = new DraftCoordinator();
    const scope = oracleScope(null);
    coordinator.open(scope, null, '');
    coordinator.revise(scope, 'What waits below the keep?');

    await expect(coordinator.requestSave(scope, async (value) => value)).rejects.toThrow(
      'Draft has no persistence target',
    );
    expect(coordinator.get<string>(scope)?.value).toBe('What waits below the keep?');
    expect(statusOf(required(coordinator.get<string>(scope), 'the Oracle draft'))).toBe('pending');
  });

  it('keeps a new entity draft under its stable client UUID until explicitly discarded', () => {
    const coordinator = new DraftCoordinator();
    const scope = newEntityScope('camp-a', 'npc', 'fd776b47-f07b-44ac-83a8-a3d45ad70c49');
    coordinator.open(scope, scope, { name: '', notes: '' });
    coordinator.revise(scope, { name: 'Sable', notes: 'Knows the old road' });

    expect(coordinator.get<{ name: string; notes: string }>(scope)?.value).toEqual({
      name: 'Sable',
      notes: 'Knows the old road',
    });
    expect(coordinator.open(scope, scope, { name: '', notes: '' }).scope).toBe(scope);
  });

  it('rejects reopening an existing scope with another target without changing the draft', () => {
    const coordinator = new DraftCoordinator();
    const scope = entityScope('camp-a', 'npc', 'mira');
    const original = coordinator.open(scope, 'entity:npc:mira', { notes: 'saved' });

    expect(() =>
      coordinator.open(scope, 'entity:npc:torvin', { notes: 'wrong target content' }),
    ).toThrow(/target/i);

    expect(coordinator.get(scope)).toBe(original);
    expect(coordinator.get(scope)).toMatchObject({
      target: 'entity:npc:mira',
      baseline: { notes: 'saved' },
      value: { notes: 'saved' },
      revision: 0,
    });
  });

  it('refreshes a fully clean existing scope from an authoritative load', () => {
    const coordinator = new DraftCoordinator();
    const scope = entityScope('camp-a', 'npc', 'mira');
    coordinator.open(scope, 'entity:npc:mira', { notes: 'first authoritative value' });

    const refreshed = coordinator.open(scope, 'entity:npc:mira', {
      notes: 'new authoritative value',
    });

    expect(refreshed).toMatchObject({
      target: 'entity:npc:mira',
      baseline: { notes: 'new authoritative value' },
      value: { notes: 'new authoritative value' },
      revision: 0,
      acknowledgedRevision: 0,
      unacknowledgedRevision: null,
      inFlight: null,
      error: null,
    });
    expect(statusOf(refreshed)).toBe('saved');
  });

  it('ignores an authoritative refresh while the local scope is saving', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ notes: string }>();
    const scope = entityScope('camp-a', 'npc', 'mira');
    coordinator.open(scope, 'entity:npc:mira', { notes: 'saved' });
    coordinator.revise(scope, { notes: 'saving locally' });

    const save = coordinator.requestSave(scope, writer.write);
    const retained = coordinator.open(scope, 'entity:npc:mira', {
      notes: 'later authoritative load',
    });

    expect(retained).toMatchObject({
      baseline: { notes: 'saved' },
      value: { notes: 'saving locally' },
      revision: 1,
    });
    expect(statusOf(retained)).toBe('saving');

    required(writer.attempts[0], 'the active write').completion.resolve({
      notes: 'saving locally',
    });
    await save;
  });

  it('ignores an authoritative refresh while the local scope has a recoverable failure', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ notes: string }>();
    const scope = entityScope('camp-a', 'npc', 'mira');
    coordinator.open(scope, 'entity:npc:mira', { notes: 'saved' });
    coordinator.revise(scope, { notes: 'failed local value' });

    const save = coordinator.requestSave(scope, writer.write);
    required(writer.attempts[0], 'the failing write').completion.reject(new Error('offline'));
    await save;
    const retained = coordinator.open(scope, 'entity:npc:mira', {
      notes: 'later authoritative load',
    });

    expect(retained).toMatchObject({
      baseline: { notes: 'saved' },
      value: { notes: 'failed local value' },
      revision: 1,
      error: 'offline',
    });
    expect(statusOf(retained)).toBe('failed');
  });

  it('ignores an authoritative refresh for an unacknowledged revision equal to its baseline', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ notes: string }>();
    const scope = entityScope('camp-a', 'npc', 'mira');
    coordinator.open(scope, 'entity:npc:mira', { notes: 'saved' });
    coordinator.revise(scope, { notes: 'first revision' });

    const save = coordinator.requestSave(scope, writer.write);
    coordinator.revise(scope, { notes: 'canonical newer revision' });
    required(writer.attempts[0], 'the older write').completion.resolve({
      notes: 'canonical newer revision',
    });
    await save;

    const retained = coordinator.open(scope, 'entity:npc:mira', {
      notes: 'later authoritative load',
    });
    expect(retained).toMatchObject({
      baseline: { notes: 'canonical newer revision' },
      value: { notes: 'canonical newer revision' },
      revision: 2,
      acknowledgedRevision: 1,
      unacknowledgedRevision: 2,
    });
    expect(statusOf(retained)).toBe('pending');
  });

  it('ignores an authoritative refresh while the same scope has a queued save', async () => {
    const coordinator = new DraftCoordinator();
    const activeWriter = new ControlledWriter<{ notes: string }>();
    const queuedWriter = new ControlledWriter<{ notes: string }>();
    const target = 'rule:initiative';
    const activeScope = 'rule:camp-a:world-guide:initiative';
    const queuedScope = 'rule:camp-b:world-guide:initiative';
    coordinator.open(activeScope, target, { notes: 'Campaign A saved' });
    coordinator.open(queuedScope, target, { notes: 'Campaign B saved' });
    coordinator.revise(activeScope, { notes: 'Campaign A saving' });
    coordinator.revise(queuedScope, { notes: 'Campaign B queued' });

    const activeSave = coordinator.requestSave(activeScope, activeWriter.write);
    const queuedSave = coordinator.requestSave(queuedScope, queuedWriter.write);

    expect(queuedWriter.attempts).toHaveLength(0);
    expect(coordinator.get(queuedScope)).toMatchObject({ inFlight: null });
    const retained = coordinator.open(queuedScope, target, {
      notes: 'Later authoritative Campaign B value',
    });
    expect(retained).toMatchObject({
      baseline: { notes: 'Campaign B saved' },
      value: { notes: 'Campaign B queued' },
      revision: 1,
      inFlight: null,
    });
    expect(statusOf(retained)).toBe('pending');

    required(activeWriter.attempts[0], 'the active target write').completion.resolve({
      notes: 'Campaign A saving',
    });
    await activeSave;

    expect(queuedWriter.attempts).toHaveLength(1);
    expect(required(queuedWriter.attempts[0], 'the queued target write').value).toEqual({
      notes: 'Campaign B queued',
    });
    required(queuedWriter.attempts[0], 'the queued target write').completion.resolve({
      notes: 'Campaign B canonical',
    });
    await queuedSave;

    expect(coordinator.get<{ notes: string }>(queuedScope)).toMatchObject({
      baseline: { notes: 'Campaign B canonical' },
      value: { notes: 'Campaign B canonical' },
      error: null,
    });
    expect(statusOf(required(coordinator.get(queuedScope), 'the acknowledged queued draft'))).toBe(
      'saved',
    );
  });

  it('protects a clean reverted draft until its obsolete queued save has been skipped', async () => {
    const coordinator = new DraftCoordinator();
    const activeWriter = new ControlledWriter<{ notes: string }>();
    const queuedWriter = new ControlledWriter<{ notes: string }>();
    const target = 'rule:initiative';
    const activeScope = 'rule:camp-a:world-guide:initiative';
    const queuedScope = 'rule:camp-b:world-guide:initiative';
    coordinator.open(activeScope, target, { notes: 'Campaign A saved' });
    coordinator.open(queuedScope, target, { notes: 'Campaign B saved' });
    coordinator.revise(activeScope, { notes: 'Campaign A saving' });
    coordinator.revise(queuedScope, { notes: 'Campaign B queued' });

    const activeSave = coordinator.requestSave(activeScope, activeWriter.write);
    const queuedSave = coordinator.requestSave(queuedScope, queuedWriter.write);
    coordinator.revise(queuedScope, { notes: 'Campaign B saved' });

    expect(coordinator.get<{ notes: string }>(queuedScope)).toMatchObject({
      baseline: { notes: 'Campaign B saved' },
      value: { notes: 'Campaign B saved' },
      revision: 2,
      unacknowledgedRevision: null,
      inFlight: null,
    });
    expect(statusOf(required(coordinator.get(queuedScope), 'the clean queued draft'))).toBe(
      'saved',
    );
    expect(queuedWriter.attempts).toHaveLength(0);

    const protectedDraft = coordinator.open(queuedScope, target, {
      notes: 'Authoritative value while queued',
    });
    expect(protectedDraft).toMatchObject({
      baseline: { notes: 'Campaign B saved' },
      value: { notes: 'Campaign B saved' },
      revision: 2,
      unacknowledgedRevision: null,
    });

    required(activeWriter.attempts[0], 'the active target write').completion.resolve({
      notes: 'Campaign A saving',
    });
    await Promise.all([activeSave, queuedSave]);

    expect(queuedWriter.attempts).toHaveLength(0);
    const refreshed = coordinator.open(queuedScope, target, {
      notes: 'Authoritative value after queue settled',
    });
    expect(refreshed).toMatchObject({
      baseline: { notes: 'Authoritative value after queue settled' },
      value: { notes: 'Authoritative value after queue settled' },
      revision: 0,
      acknowledgedRevision: 0,
      unacknowledgedRevision: null,
      inFlight: null,
      error: null,
    });
    expect(statusOf(refreshed)).toBe('saved');
  });
});

describe('DraftCoordinator draft discovery', () => {
  it('returns frozen snapshot records only for the exact requested prefix', () => {
    const coordinator = new DraftCoordinator();
    const firstMatch = newEntityScope('camp-a', 'npc', 'draft-one');
    const secondMatch = newEntityScope('camp-a', 'npc', 'draft-two');
    const neighboringKind = newEntityScope('camp-a', 'location', 'draft-one');
    const neighboringCampaign = newEntityScope('camp-b', 'npc', 'draft-one');
    const lookalikePrefix = 'entity-new:camp-a:npc-extra:draft-one';

    coordinator.open(firstMatch, firstMatch, { name: '', notes: '' });
    coordinator.revise(firstMatch, { name: 'Sable', notes: 'First retained value' });
    coordinator.open(secondMatch, secondMatch, { name: '', notes: '' });
    coordinator.revise(secondMatch, { name: 'Mira', notes: 'Second retained value' });
    coordinator.open(neighboringKind, neighboringKind, { name: 'Moon Gate', notes: '' });
    coordinator.open(neighboringCampaign, neighboringCampaign, { name: 'Elsewhere', notes: '' });
    coordinator.open(lookalikePrefix, lookalikePrefix, { name: 'Lookalike', notes: '' });

    const listed: ReadonlyArray<DraftRecord<{ name: string; notes: string }>> =
      coordinator.listByPrefix<{ name: string; notes: string }>('entity-new:camp-a:npc:');

    expect(listed.map(({ scope }) => scope)).toEqual([firstMatch, secondMatch]);
    expect(Object.isFrozen(listed)).toBe(true);
    expect(listed.every((record) => Object.isFrozen(record))).toBe(true);
  });

  it('does not expose mutable or live coordinator records through prefix discovery', () => {
    const coordinator = new DraftCoordinator();
    const scope = newEntityScope('camp-a', 'npc', 'draft-one');
    coordinator.open(scope, scope, { name: '', notes: '' });
    coordinator.revise(scope, { name: 'Sable', notes: 'Retained value' });

    const listed: ReadonlyArray<DraftRecord<{ name: string; notes: string }>> =
      coordinator.listByPrefix<{ name: string; notes: string }>('entity-new:camp-a:npc:');
    const snapshot = required(listed[0], 'the discovered draft snapshot');

    expect(() => Object.assign(snapshot, { revision: 99, error: 'mutated externally' })).toThrow(
      TypeError,
    );
    expect(coordinator.get(scope)).toMatchObject({ revision: 1, error: null });

    coordinator.revise(scope, { name: 'Sable', notes: 'A later coordinator revision' });

    expect(snapshot).toMatchObject({
      revision: 1,
      value: { name: 'Sable', notes: 'Retained value' },
    });
    expect(coordinator.get(scope)).toMatchObject({
      revision: 2,
      value: { name: 'Sable', notes: 'A later coordinator revision' },
    });
  });
});

describe('DraftCoordinator bounded retention', () => {
  it('releases only a clean scope when its adapter no longer references it', () => {
    const coordinator = new DraftCoordinator();
    const cleanScope = entityScope('camp-a', 'npc', 'clean');
    const dirtyScope = entityScope('camp-a', 'npc', 'dirty');
    const unrelatedScope = entityScope('camp-b', 'npc', 'clean');
    coordinator.open(cleanScope, 'entity:npc:clean', { notes: 'saved' });
    coordinator.open(dirtyScope, 'entity:npc:dirty', { notes: 'saved' });
    coordinator.revise(dirtyScope, { notes: 'keep me' });
    coordinator.open(unrelatedScope, 'entity:npc:clean', { notes: 'elsewhere' });

    expect(coordinator.release(dirtyScope)).toBe('retained-at-risk');
    expect(coordinator.release(cleanScope)).toBe('released');
    expect(coordinator.release(cleanScope)).toBe('missing');
    expect(coordinator.get(cleanScope)).toBeUndefined();
    expect(coordinator.get(dirtyScope)?.value).toEqual({ notes: 'keep me' });
    expect(coordinator.get(unrelatedScope)?.value).toEqual({ notes: 'elsewhere' });
  });

  it('defers requested release until an active save settles without an owner', async () => {
    const coordinator = new DraftCoordinator();
    const completion = deferred<{ notes: string }>();
    const scope = entityScope('camp-a', 'npc', 'mira');
    coordinator.open(scope, 'entity:npc:mira', { notes: 'Saved' });
    const releaseLease = coordinator.acquireLease(scope);
    coordinator.revise(scope, { notes: 'Saving' });
    const save = coordinator.requestSave(scope, () => completion.promise);

    expect(coordinator.release(scope)).toBe('retained-at-risk');
    releaseLease();
    completion.resolve({ notes: 'Saving' });
    await save;

    expect(coordinator.get(scope)).toBeUndefined();
  });

  it('keeps a settled draft while a replacement adapter owns its lease', async () => {
    const coordinator = new DraftCoordinator();
    const completion = deferred<{ notes: string }>();
    const scope = entityScope('camp-a', 'npc', 'mira');
    coordinator.open(scope, 'entity:npc:mira', { notes: 'Saved' });
    const releaseOldLease = coordinator.acquireLease(scope);
    coordinator.revise(scope, { notes: 'Saving' });
    const save = coordinator.requestSave(scope, () => completion.promise);
    const releaseReplacementLease = coordinator.acquireLease(scope);

    expect(coordinator.release(scope)).toBe('retained-at-risk');
    releaseOldLease();
    completion.resolve({ notes: 'Saving' });
    await save;
    expect(coordinator.get(scope)).toBeDefined();

    expect(coordinator.release(scope)).toBe('retained-at-risk');
    releaseReplacementLease();
    expect(coordinator.get(scope)).toBeUndefined();
  });

  it('never releases saving, queued, or failed scopes', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ notes: string }>();
    const queuedWriter = new ControlledWriter<{ notes: string }>();
    const savingScope = 'rule:camp-a:book:shared';
    const queuedScope = 'rule:camp-b:book:shared';
    coordinator.open(savingScope, 'rule:shared', { notes: 'A saved' });
    coordinator.open(queuedScope, 'rule:shared', { notes: 'B saved' });
    coordinator.revise(savingScope, { notes: 'A saving' });
    coordinator.revise(queuedScope, { notes: 'B queued' });
    const saving = coordinator.requestSave(savingScope, writer.write);
    const queued = coordinator.requestSave(queuedScope, queuedWriter.write);

    expect(coordinator.release(savingScope)).toBe('retained-at-risk');
    expect(coordinator.release(queuedScope)).toBe('retained-at-risk');

    required(writer.attempts[0], 'the active write').completion.resolve({ notes: 'A saving' });
    await saving;
    required(queuedWriter.attempts[0], 'the queued write').completion.reject(new Error('offline'));
    await queued;
    expect(coordinator.release(queuedScope)).toBe('retained-at-risk');
  });

  it.each(['source alias', 'destination scope'] as const)(
    'releases a clean promoted lifecycle unit through its %s',
    async (releaseThrough) => {
      const coordinator = new DraftCoordinator();
      const firstWriter = new ControlledWriter<{ name: string; notes: string }>();
      const secondWriter = new ControlledWriter<{ name: string; notes: string }>();
      const firstSource = newEntityScope('camp-a', 'npc', 'first-client');
      const firstDestination = entityScope('camp-a', 'npc', 'first-saved');
      const secondSource = newEntityScope('camp-b', 'npc', 'second-client');
      const secondDestination = entityScope('camp-b', 'npc', 'second-saved');

      for (const [source, destination, target, name, writer] of [
        [firstSource, firstDestination, 'entity:npc:first-saved', 'Sable', firstWriter],
        [secondSource, secondDestination, 'entity:npc:second-saved', 'Mira', secondWriter],
      ] as const) {
        coordinator.open(source, source, { name: '', notes: '' });
        coordinator.revise(source, { name, notes: '' });
        const create = coordinator.requestSave(source, writer.write);
        required(writer.attempts[0], `${name} create write`).completion.resolve({
          name,
          notes: '',
        });
        await create;
        coordinator.remapAfterCreate(source, destination, target);
      }

      const requestedScope = releaseThrough === 'source alias' ? firstSource : firstDestination;
      expect(coordinator.release(requestedScope)).toBe('released');
      expect(coordinator.get(firstDestination)).toBeUndefined();
      expect(coordinator.resolveScope(firstSource)).toBe(firstSource);
      expect(coordinator.get(secondDestination)).toBeDefined();
      expect(coordinator.resolveScope(secondSource)).toBe(secondDestination);
    },
  );

  it('releases only eligible clean records under the exact prefix', () => {
    const coordinator = new DraftCoordinator();
    const clean = entityScope('camp-a', 'npc', 'clean');
    const dirty = entityScope('camp-a', 'npc', 'dirty');
    const anotherKind = entityScope('camp-a', 'location', 'clean');
    const anotherCampaign = entityScope('camp-b', 'npc', 'clean');
    coordinator.open(clean, 'entity:npc:clean', { notes: 'saved' });
    coordinator.open(dirty, 'entity:npc:dirty', { notes: 'saved' });
    coordinator.revise(dirty, { notes: 'draft' });
    coordinator.open(anotherKind, 'entity:location:clean', { notes: 'location' });
    coordinator.open(anotherCampaign, 'entity:npc:clean', { notes: 'elsewhere' });

    expect(coordinator.releasePrefix('entity:camp-a:npc:')).toBe(1);
    expect(coordinator.get(clean)).toBeUndefined();
    expect(coordinator.get(dirty)).toBeDefined();
    expect(coordinator.get(anotherKind)).toBeDefined();
    expect(coordinator.get(anotherCampaign)).toBeDefined();
  });

  it('retains a clean source while its create promotion has an unresolved issue', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ name: string; notes: string }>();
    const sourceScope = newEntityScope('camp-a', 'npc', 'client');
    const destinationScope = entityScope('camp-a', 'npc', 'saved-id');
    coordinator.open(sourceScope, sourceScope, { name: '', notes: '' });
    coordinator.revise(sourceScope, { name: 'Sable', notes: '' });
    const create = coordinator.requestSave(sourceScope, writer.write);
    required(writer.attempts[0], 'the create write').completion.resolve({
      name: 'Sable',
      notes: '',
    });
    await create;
    coordinator.open(destinationScope, 'entity:npc:different-target', {
      name: 'Sable',
      notes: 'different authority',
    });

    expect(
      coordinator.tryRemapAfterCreate(sourceScope, destinationScope, 'entity:npc:saved-id'),
    ).toEqual({ outcome: 'blocked', reason: 'destination-target-mismatch' });
    expect(coordinator.release(sourceScope)).toBe('retained-at-risk');
    expect(coordinator.get(sourceScope)).toBeDefined();
  });

  it('purges only confirmed campaign prefixes and all associated metadata', async () => {
    const coordinator = new DraftCoordinator();
    const campaignAScope = entityScope('camp-a', 'npc', 'saved-id');
    const campaignANewScope = newEntityScope('camp-a', 'npc', 'client');
    const campaignB = entityScope('camp-b', 'npc', 'saved-id');
    const lookalikeCampaign = 'oracle:camp-ab';
    const createWriter = new ControlledWriter<{ name: string; notes: string }>();
    coordinator.open(campaignANewScope, campaignANewScope, { name: '', notes: '' });
    coordinator.revise(campaignANewScope, { name: 'Sable', notes: '' });
    const create = coordinator.requestSave(campaignANewScope, createWriter.write);
    required(createWriter.attempts[0], 'the create write').completion.resolve({
      name: 'Sable',
      notes: '',
    });
    await create;
    coordinator.remapAfterCreate(campaignANewScope, campaignAScope, 'entity:npc:saved-id');
    coordinator.open(campaignB, 'entity:npc:saved-id', { name: 'Elsewhere', notes: '' });
    coordinator.open(lookalikeCampaign, null, 'Keep this campaign');

    expect(
      coordinator.removeAfterDeletePrefixes([
        'oracle:camp-a',
        'entity:camp-a:',
        'entity-new:camp-a:',
        'session:camp-a:',
        'rule:camp-a:',
      ]),
    ).toBe(1);
    expect(coordinator.get(campaignAScope)).toBeUndefined();
    expect(coordinator.resolveScope(campaignANewScope)).toBe(campaignANewScope);
    expect(coordinator.getCreatePromotionIssue(campaignANewScope)).toBeUndefined();
    expect(coordinator.get(campaignB)).toBeDefined();
    expect(coordinator.get(lookalikeCampaign)?.value).toBe('Keep this campaign');
  });

  it('refuses a campaign purge while a matching scope is actively saving', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ notes: string }>();
    const scope = entityScope('camp-a', 'npc', 'mira');
    coordinator.open(scope, 'entity:npc:mira', { notes: 'saved' });
    coordinator.revise(scope, { notes: 'saving' });
    const save = coordinator.requestSave(scope, writer.write);

    expect(coordinator.hasActiveWritesByPrefixes(['entity:camp-a:'])).toBe(true);
    expect(coordinator.removeAfterDeletePrefixes(['entity:camp-a:'])).toBe('blocked-active-save');
    expect(coordinator.get(scope)?.value).toEqual({ notes: 'saving' });

    required(writer.attempts[0], 'the active write').completion.resolve({ notes: 'saving' });
    await save;
  });
});

describe('DraftCoordinator create acknowledgment remapping', () => {
  it('reactively resolves a remapped client scope without retargeting stale mutations', async () => {
    const coordinator = new DraftCoordinator();
    const createWriter = new ControlledWriter<{ name: string; notes: string }>();
    const staleWriter = new ControlledWriter<{ name: string; notes: string }>();
    const clientScope = newEntityScope('camp-a', 'npc', 'client-draft');
    const existingScope = entityScope('camp-a', 'npc', 'sable');
    const existingTarget = 'entity:npc:sable';
    coordinator.open(clientScope, clientScope, { name: '', notes: '' });
    coordinator.revise(clientScope, { name: 'Captain Sable', notes: 'Create revision' });

    expect(coordinator.resolveScope(clientScope)).toBe(clientScope);

    const create = coordinator.requestSave(clientScope, createWriter.write);
    required(createWriter.attempts[0], 'the create attempt').completion.resolve({
      name: 'Captain Sable',
      notes: 'Canonical create revision',
    });
    await create;

    coordinator.remapAfterCreate(clientScope, existingScope, existingTarget);

    expect(coordinator.resolveScope(clientScope)).toBe(existingScope);
    expect(coordinator.resolveScope(existingScope)).toBe(existingScope);
    expect(coordinator.get(clientScope)).toBeUndefined();
    expect(coordinator.get(existingScope)).toMatchObject({
      scope: existingScope,
      target: existingTarget,
      baseline: { name: 'Captain Sable', notes: 'Canonical create revision' },
      value: { name: 'Captain Sable', notes: 'Canonical create revision' },
    });
    expect(statusOf(required(coordinator.get(existingScope), 'the remapped draft'))).toBe('saved');

    expect(() =>
      coordinator.revise(clientScope, { name: 'Stale edit', notes: 'Must not be redirected' }),
    ).toThrow(/not open/i);
    await expect(coordinator.requestSave(clientScope, staleWriter.write)).rejects.toThrow(
      /not open/i,
    );
    expect(staleWriter.attempts).toHaveLength(0);
    expect(coordinator.get(existingScope)).toMatchObject({
      target: existingTarget,
      value: { name: 'Captain Sable', notes: 'Canonical create revision' },
    });
  });

  it('moves an acknowledged create to its backend scope as a saved canonical draft', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ name: string; notes: string }>();
    const clientScope = newEntityScope('camp-a', 'npc', 'client-draft');
    const existingScope = entityScope('camp-a', 'npc', 'sable');
    const existingTarget = 'entity:npc:sable';
    coordinator.open(clientScope, clientScope, { name: '', notes: '' });
    coordinator.revise(clientScope, { name: 'Captain Sable', notes: 'Draft notes' });

    const create = coordinator.requestSave(clientScope, writer.write);
    required(writer.attempts[0], 'the create attempt').completion.resolve({
      name: 'Captain Sable',
      notes: 'Canonical notes',
    });
    await create;

    const remapped = coordinator.remapAfterCreate<{
      name: string;
      notes: string;
    }>(clientScope, existingScope, existingTarget);

    expect(coordinator.get(clientScope)).toBeUndefined();
    expect(coordinator.get(existingScope)).toBe(remapped);
    expect(remapped).toMatchObject({
      scope: existingScope,
      target: existingTarget,
      baseline: { name: 'Captain Sable', notes: 'Canonical notes' },
      value: { name: 'Captain Sable', notes: 'Canonical notes' },
      revision: 1,
      acknowledgedRevision: 1,
      unacknowledgedRevision: null,
      inFlight: null,
      error: null,
    });
    expect(statusOf(remapped)).toBe('saved');

    expect(() =>
      coordinator.revise(clientScope, { name: 'Stale client edit', notes: 'Must not move' }),
    ).toThrow(/not open/i);
    await expect(coordinator.requestSave(clientScope, writer.write)).rejects.toThrow(/not open/i);
    expect(coordinator.get(existingScope)).toBe(remapped);
  });

  it('preserves a newer edit as pending when an older create acknowledgment is remapped', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ name: string; notes: string }>();
    const clientScope = newEntityScope('camp-a', 'npc', 'client-draft');
    const existingScope = entityScope('camp-a', 'npc', 'sable');
    const existingTarget = 'entity:npc:sable';
    coordinator.open(clientScope, clientScope, { name: '', notes: '' });
    coordinator.revise(clientScope, { name: 'Captain Sable', notes: 'Create revision' });

    const create = coordinator.requestSave(clientScope, writer.write);
    coordinator.revise(clientScope, {
      name: 'Captain Sable',
      notes: 'Newer edit during create',
    });
    required(writer.attempts[0], 'the older create attempt').completion.resolve({
      name: 'Captain Sable',
      notes: 'Canonical create revision',
    });
    await create;

    const remapped = coordinator.remapAfterCreate<{
      name: string;
      notes: string;
    }>(clientScope, existingScope, existingTarget);

    expect(coordinator.get(clientScope)).toBeUndefined();
    expect(remapped).toMatchObject({
      scope: existingScope,
      target: existingTarget,
      baseline: { name: 'Captain Sable', notes: 'Canonical create revision' },
      value: { name: 'Captain Sable', notes: 'Newer edit during create' },
      revision: 2,
      acknowledgedRevision: 1,
      unacknowledgedRevision: 2,
      inFlight: null,
      error: null,
    });
    expect(coordinator.resolveScope(clientScope)).toBe(existingScope);
    expect(statusOf(remapped)).toBe('pending');
    expect(coordinator.atRiskCount()).toBe(1);
  });

  it('atomically promotes an acknowledged create over a clean exact-target destination', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ name: string; notes: string }>();
    const clientScope = newEntityScope('camp-a', 'npc', 'client-draft');
    const destinationScope = entityScope('camp-a', 'npc', 'sable');
    const destinationTarget = 'entity:npc:sable';
    coordinator.open(clientScope, clientScope, { name: '', notes: '' });
    coordinator.revise(clientScope, { name: 'Captain Sable', notes: 'Create revision' });

    const create = coordinator.requestSave(clientScope, writer.write);
    coordinator.revise(clientScope, {
      name: 'Captain Sable',
      notes: 'Newer source revision',
    });
    required(writer.attempts[0], 'the create attempt').completion.resolve({
      name: 'Captain Sable',
      notes: 'Canonical create revision',
    });
    await create;

    coordinator.open(destinationScope, destinationTarget, {
      name: 'Captain Sable',
      notes: 'Canonical create revision',
    });
    const remapped = coordinator.remapAfterCreate<{
      name: string;
      notes: string;
    }>(clientScope, destinationScope, destinationTarget);

    expect(coordinator.get(clientScope)).toBeUndefined();
    expect(coordinator.resolveScope(clientScope)).toBe(destinationScope);
    expect(coordinator.get(destinationScope)).toBe(remapped);
    expect(remapped).toMatchObject({
      scope: destinationScope,
      target: destinationTarget,
      baseline: { name: 'Captain Sable', notes: 'Canonical create revision' },
      value: { name: 'Captain Sable', notes: 'Newer source revision' },
      acknowledgedRevision: 1,
      unacknowledgedRevision: 2,
      inFlight: null,
      error: null,
    });
    expect(statusOf(remapped)).toBe('pending');
  });

  it('does not promote an older create acknowledgment over a destination with newer acknowledged authority', async () => {
    interface EntityValue extends Readonly<Record<string, DraftValue>> {
      readonly name: string;
      readonly notes: string;
    }

    const coordinator = new DraftCoordinator();
    const createWriter = new ControlledWriter<EntityValue>();
    const updateWriter = new ControlledWriter<EntityValue>();
    const clientScope = newEntityScope('camp-a', 'npc', 'client-draft');
    const destinationScope = entityScope('camp-a', 'npc', 'created-record');
    const destinationTarget = 'entity:npc:created-record';
    coordinator.open(clientScope, clientScope, { name: '', notes: '' });
    coordinator.revise(clientScope, { name: 'Captain Sable', notes: 'Create revision one' });

    const create = coordinator.requestSave<EntityValue>(clientScope, createWriter.write);
    expect(coordinator.get(clientScope)?.inFlight?.id).toBe(1);
    coordinator.revise(clientScope, {
      name: 'Captain Sable',
      notes: 'Source revision two remains recoverable',
    });

    // The committed Create appears through an authoritative list load before its
    // older acknowledgment is delivered to the initiating editor.
    coordinator.open(destinationScope, destinationTarget, {
      name: 'Captain Sable',
      notes: 'Create revision one',
    });
    coordinator.revise(destinationScope, {
      name: 'Captain Sable',
      notes: 'Transient destination edit',
    });
    const update = coordinator.requestSave<EntityValue>(destinationScope, updateWriter.write);
    expect(coordinator.get(destinationScope)?.inFlight?.id).toBe(2);
    // Canonicalization returns the same content as Create r1. Causal authority,
    // not value equality, must still identify this as the newer acknowledgment.
    required(updateWriter.attempts[0], 'the newer destination attempt').completion.resolve({
      name: 'Captain Sable',
      notes: 'Create revision one',
    });
    await update;

    const newerDestination = required(
      coordinator.get<EntityValue>(destinationScope),
      'the destination with newer acknowledged authority',
    );
    expect(statusOf(newerDestination)).toBe('saved');

    required(createWriter.attempts[0], 'the older create attempt').completion.resolve({
      name: 'Captain Sable',
      notes: 'Create revision one',
    });
    await create;

    const sourceAfterOlderAcknowledgment = required(
      coordinator.get<EntityValue>(clientScope),
      'the source after its older acknowledgment',
    );
    expect(sourceAfterOlderAcknowledgment).toMatchObject({
      acknowledgedRevision: 1,
      unacknowledgedRevision: 2,
      value: { name: 'Captain Sable', notes: 'Source revision two remains recoverable' },
    });
    expect(statusOf(sourceAfterOlderAcknowledgment)).toBe('pending');

    const result = coordinator.tryRemapAfterCreate<EntityValue>(
      clientScope,
      destinationScope,
      destinationTarget,
    );

    expect(result).toEqual({
      outcome: 'blocked',
      reason: 'destination-newer-acknowledgment',
    });
    expect(coordinator.get(clientScope)).toBe(sourceAfterOlderAcknowledgment);
    expect(coordinator.get(destinationScope)).toBe(newerDestination);
    expect(coordinator.get(destinationScope)).toMatchObject({
      lastAcknowledgedAttemptId: 2,
      value: { name: 'Captain Sable', notes: 'Create revision one' },
    });
    expect(coordinator.get(clientScope)).toMatchObject({ lastAcknowledgedAttemptId: 1 });
    expect(coordinator.getCreatePromotionIssue(clientScope)).toMatchObject({
      sourceScope: clientScope,
      destinationScope,
      destinationTarget,
      reason: 'destination-newer-acknowledgment',
    });
    expect(coordinator.resolveScope(clientScope)).toBe(clientScope);
    expect(coordinator.atRiskCount()).toBe(1);
  });

  it('converges a settled Create without newer source work onto newer destination authority', async () => {
    const coordinator = new DraftCoordinator();
    const createWriter = new ControlledWriter<{ name: string; notes: string }>();
    const updateWriter = new ControlledWriter<{ name: string; notes: string }>();
    const sourceScope = newEntityScope('camp-a', 'npc', 'client-draft');
    const destinationScope = entityScope('camp-a', 'npc', 'created-record');
    const destinationTarget = 'entity:npc:created-record';
    coordinator.open(sourceScope, sourceScope, { name: '', notes: '' });
    coordinator.revise(sourceScope, { name: 'Captain Sable', notes: 'Create revision' });

    const create = coordinator.requestSave(sourceScope, createWriter.write);
    coordinator.open(
      destinationScope,
      destinationTarget,
      { name: 'Captain Sable', notes: 'Create revision' },
      'authoritative-list',
    );
    coordinator.revise(destinationScope, {
      name: 'Captain Sable',
      notes: 'Newer saved destination',
    });
    const update = coordinator.requestSave(destinationScope, updateWriter.write);
    required(updateWriter.attempts[0], 'the newer destination update').completion.resolve({
      name: 'Captain Sable',
      notes: 'Newer saved destination',
    });
    await update;
    const destinationBeforeCreate = required(
      coordinator.get<{ name: string; notes: string }>(destinationScope),
      'the newer destination',
    );

    required(createWriter.attempts[0], 'the older Create').completion.resolve({
      name: 'Captain Sable',
      notes: 'Create revision',
    });
    await create;
    const result = coordinator.tryRemapAfterCreate<{ name: string; notes: string }>(
      sourceScope,
      destinationScope,
      destinationTarget,
    );

    expect(result).toEqual({
      outcome: 'promoted',
      resolution: 'destination-converged',
      draft: destinationBeforeCreate,
    });
    expect(coordinator.get(sourceScope)).toBeUndefined();
    expect(coordinator.get(destinationScope)).toBe(destinationBeforeCreate);
    expect(coordinator.resolveScope(sourceScope)).toBe(destinationScope);
    expect(coordinator.getCreatePromotionIssue(sourceScope)).toBeUndefined();
    expect(coordinator.atRiskCount()).toBe(0);
    expect(createWriter.attempts).toHaveLength(1);
    expect(updateWriter.attempts).toHaveLength(1);
  });

  it('retains newer acknowledged authority through a clean authoritative list refresh', async () => {
    const coordinator = new DraftCoordinator();
    const createWriter = new ControlledWriter<{ name: string; notes: string }>();
    const updateWriter = new ControlledWriter<{ name: string; notes: string }>();
    const sourceScope = newEntityScope('camp-a', 'npc', 'client-draft');
    const destinationScope = entityScope('camp-a', 'npc', 'created-record');
    const destinationTarget = 'entity:npc:created-record';
    coordinator.open(sourceScope, sourceScope, { name: '', notes: '' });
    coordinator.revise(sourceScope, { name: 'Captain Sable', notes: 'Create revision' });

    const create = coordinator.requestSave(sourceScope, createWriter.write);
    coordinator.revise(sourceScope, {
      name: 'Captain Sable',
      notes: 'Pending source revision two',
    });
    coordinator.open(
      destinationScope,
      destinationTarget,
      { name: 'Captain Sable', notes: 'Create revision' },
      'authoritative-list',
    );
    coordinator.revise(destinationScope, {
      name: 'Captain Sable',
      notes: 'Destination update',
    });
    const update = coordinator.requestSave(destinationScope, updateWriter.write);
    required(updateWriter.attempts[0], 'the newer destination update').completion.resolve({
      name: 'Captain Sable',
      notes: 'Destination update',
    });
    await update;

    const refreshed = coordinator.open(
      destinationScope,
      destinationTarget,
      { name: 'Captain Sable', notes: 'Later clean list content' },
      'authoritative-list',
    );
    expect(refreshed).toMatchObject({
      value: { name: 'Captain Sable', notes: 'Later clean list content' },
      lastAcknowledgedAttemptId: 2,
    });

    required(createWriter.attempts[0], 'the older Create').completion.resolve({
      name: 'Captain Sable',
      notes: 'Create revision',
    });
    await create;
    const result = coordinator.tryRemapAfterCreate(
      sourceScope,
      destinationScope,
      destinationTarget,
    );

    expect(result).toEqual({
      outcome: 'blocked',
      reason: 'destination-newer-acknowledgment',
    });
    expect(coordinator.get(destinationScope)).toBe(refreshed);
    expect(coordinator.get(destinationScope)).toMatchObject({
      value: { name: 'Captain Sable', notes: 'Later clean list content' },
      lastAcknowledgedAttemptId: 2,
    });
    expect(coordinator.get(sourceScope)).toMatchObject({
      value: { name: 'Captain Sable', notes: 'Pending source revision two' },
      lastAcknowledgedAttemptId: 1,
    });
  });

  it('keeps the saved destination when explicitly resolving a Create promotion conflict', async () => {
    const coordinator = new DraftCoordinator();
    const createWriter = new ControlledWriter<{ name: string; notes: string }>();
    const updateWriter = new ControlledWriter<{ name: string; notes: string }>();
    const sourceScope = newEntityScope('camp-a', 'npc', 'client-draft');
    const destinationScope = entityScope('camp-a', 'npc', 'created-record');
    const destinationTarget = 'entity:npc:created-record';
    const unrelatedScope = entityScope('camp-b', 'npc', 'other-record');
    coordinator.open(sourceScope, sourceScope, { name: '', notes: '' });
    coordinator.revise(sourceScope, { name: 'Captain Sable', notes: 'Create revision' });
    coordinator.open(unrelatedScope, 'entity:npc:other-record', {
      name: 'Other campaign NPC',
      notes: 'Unrelated draft',
    });

    const create = coordinator.requestSave(sourceScope, createWriter.write);
    coordinator.revise(sourceScope, { name: 'Captain Sable', notes: 'Source revision two' });
    coordinator.open(destinationScope, destinationTarget, {
      name: 'Captain Sable',
      notes: 'Create revision',
    });
    coordinator.revise(destinationScope, {
      name: 'Captain Sable',
      notes: 'Destination authority',
    });
    const update = coordinator.requestSave(destinationScope, updateWriter.write);
    required(updateWriter.attempts[0], 'the destination update').completion.resolve({
      name: 'Captain Sable',
      notes: 'Destination authority',
    });
    await update;
    const destinationBeforeResolution = coordinator.get(destinationScope);
    const unrelatedBeforeResolution = coordinator.get(unrelatedScope);
    required(createWriter.attempts[0], 'the Create').completion.resolve({
      name: 'Captain Sable',
      notes: 'Create revision',
    });
    await create;
    expect(
      coordinator.tryRemapAfterCreate(sourceScope, destinationScope, destinationTarget),
    ).toEqual({ outcome: 'blocked', reason: 'destination-newer-acknowledgment' });

    const result = coordinator.resolveCreatePromotion(sourceScope, 'keep-destination');

    expect(result).toMatchObject({
      outcome: 'promoted',
      resolution: 'destination-converged',
    });
    expect(coordinator.get(sourceScope)).toBeUndefined();
    expect(coordinator.get(destinationScope)).toBe(destinationBeforeResolution);
    expect(coordinator.resolveScope(sourceScope)).toBe(destinationScope);
    expect(coordinator.getCreatePromotionIssue(sourceScope)).toBeUndefined();
    expect(coordinator.get(unrelatedScope)).toBe(unrelatedBeforeResolution);
    expect(createWriter.attempts).toHaveLength(1);
    expect(updateWriter.attempts).toHaveLength(1);
  });

  it('keeps the pending source at the returned Update target when resolving a Create conflict', async () => {
    const coordinator = new DraftCoordinator();
    const createWriter = new ControlledWriter<{ name: string; notes: string }>();
    const updateWriter = new ControlledWriter<{ name: string; notes: string }>();
    const sourceScope = newEntityScope('camp-a', 'npc', 'client-draft');
    const destinationScope = entityScope('camp-a', 'npc', 'created-record');
    const destinationTarget = 'entity:npc:created-record';
    const unrelatedScope = entityScope('camp-b', 'location', 'moon-gate');
    coordinator.open(sourceScope, sourceScope, { name: '', notes: '' });
    coordinator.revise(sourceScope, { name: 'Captain Sable', notes: 'Create revision' });
    coordinator.open(unrelatedScope, 'entity:location:moon-gate', {
      name: 'Moon Gate',
      notes: 'Unrelated campaign draft',
    });

    const create = coordinator.requestSave(sourceScope, createWriter.write);
    coordinator.revise(sourceScope, { name: 'Captain Sable', notes: 'Keep my pending draft' });
    coordinator.open(destinationScope, destinationTarget, {
      name: 'Captain Sable',
      notes: 'Create revision',
    });
    coordinator.revise(destinationScope, {
      name: 'Captain Sable',
      notes: 'Destination authority',
    });
    const update = coordinator.requestSave(destinationScope, updateWriter.write);
    required(updateWriter.attempts[0], 'the destination update').completion.resolve({
      name: 'Captain Sable',
      notes: 'Destination authority',
    });
    await update;
    const unrelatedBeforeResolution = coordinator.get(unrelatedScope);
    required(createWriter.attempts[0], 'the Create').completion.resolve({
      name: 'Captain Sable',
      notes: 'Create revision',
    });
    await create;
    expect(
      coordinator.tryRemapAfterCreate(sourceScope, destinationScope, destinationTarget),
    ).toEqual({ outcome: 'blocked', reason: 'destination-newer-acknowledgment' });

    const result = coordinator.resolveCreatePromotion<{ name: string; notes: string }>(
      sourceScope,
      'keep-source',
    );

    expect(result).toMatchObject({
      outcome: 'promoted',
      resolution: 'source-promoted',
      draft: {
        scope: destinationScope,
        target: destinationTarget,
        baseline: { name: 'Captain Sable', notes: 'Create revision' },
        value: { name: 'Captain Sable', notes: 'Keep my pending draft' },
        acknowledgedRevision: 1,
        unacknowledgedRevision: 2,
      },
    });
    if (result.outcome !== 'promoted') throw new Error('Expected source promotion');
    expect(statusOf(result.draft)).toBe('pending');
    expect(coordinator.get(sourceScope)).toBeUndefined();
    expect(coordinator.get(destinationScope)).toBe(result.draft);
    expect(coordinator.resolveScope(sourceScope)).toBe(destinationScope);
    expect(coordinator.getCreatePromotionIssue(sourceScope)).toBeUndefined();
    expect(coordinator.get(unrelatedScope)).toBe(unrelatedBeforeResolution);
    expect(createWriter.attempts).toHaveLength(1);
    expect(updateWriter.attempts).toHaveLength(1);
  });

  it.each(['dirty', 'failed', 'saving', 'queued'] as const)(
    'does not absorb an exact-target destination while it is %s',
    async (destinationState) => {
      interface EntityValue extends Readonly<Record<string, DraftValue>> {
        readonly name: string;
        readonly notes: string;
      }
      const coordinator = new DraftCoordinator();
      const createWriter = new ControlledWriter<EntityValue>();
      const destinationWriter = new ControlledWriter<EntityValue>();
      const blockerWriter = new ControlledWriter<EntityValue>();
      const clientScope = newEntityScope('camp-a', 'npc', 'client-draft');
      const destinationScope = entityScope('camp-a', 'npc', 'sable');
      const destinationTarget = 'entity:npc:sable';
      coordinator.open(clientScope, clientScope, { name: '', notes: '' });
      coordinator.revise(clientScope, { name: 'Captain Sable', notes: 'Create revision' });
      const create = coordinator.requestSave<EntityValue>(clientScope, createWriter.write);
      required(createWriter.attempts[0], 'the create attempt').completion.resolve({
        name: 'Captain Sable',
        notes: 'Canonical create revision',
      });
      await create;

      coordinator.open(destinationScope, destinationTarget, {
        name: 'Captain Sable',
        notes: 'Destination baseline',
      });

      let settleDestination = async () => {};
      if (destinationState === 'dirty') {
        coordinator.revise(destinationScope, {
          name: 'Captain Sable',
          notes: 'Dirty destination revision',
        });
      } else if (destinationState === 'failed') {
        coordinator.revise(destinationScope, {
          name: 'Captain Sable',
          notes: 'Failed destination revision',
        });
        const failedSave = coordinator.requestSave<EntityValue>(
          destinationScope,
          destinationWriter.write,
        );
        required(destinationWriter.attempts[0], 'the failed destination attempt').completion.reject(
          new Error('destination unavailable'),
        );
        await failedSave;
      } else if (destinationState === 'saving') {
        coordinator.revise(destinationScope, {
          name: 'Captain Sable',
          notes: 'Saving destination revision',
        });
        const saving = coordinator.requestSave<EntityValue>(
          destinationScope,
          destinationWriter.write,
        );
        settleDestination = async () => {
          required(
            destinationWriter.attempts[0],
            'the saving destination attempt',
          ).completion.resolve({ name: 'Captain Sable', notes: 'Saving destination revision' });
          await saving;
        };
      } else {
        const blockerScope = 'entity-list-reconciliation:blocker';
        coordinator.open(blockerScope, destinationTarget, {
          name: 'Blocker',
          notes: 'Blocker baseline',
        });
        coordinator.revise(blockerScope, { name: 'Blocker', notes: 'Blocker saving' });
        const blockerSave = coordinator.requestSave<EntityValue>(blockerScope, blockerWriter.write);
        coordinator.revise(destinationScope, {
          name: 'Captain Sable',
          notes: 'Queued destination revision',
        });
        const queuedSave = coordinator.requestSave<EntityValue>(
          destinationScope,
          destinationWriter.write,
        );
        expect(destinationWriter.attempts).toHaveLength(0);
        settleDestination = async () => {
          required(blockerWriter.attempts[0], 'the blocker attempt').completion.resolve({
            name: 'Blocker',
            notes: 'Blocker saving',
          });
          await vi.waitFor(() => expect(destinationWriter.attempts).toHaveLength(1));
          required(
            destinationWriter.attempts[0],
            'the queued destination attempt',
          ).completion.resolve({ name: 'Captain Sable', notes: 'Queued destination revision' });
          await Promise.all([blockerSave, queuedSave]);
        };
      }

      const sourceBefore = coordinator.get(clientScope);
      const destinationBefore = coordinator.get(destinationScope);
      expect(() =>
        coordinator.remapAfterCreate(clientScope, destinationScope, destinationTarget),
      ).toThrow(/occupied|destination|at.risk|saving|queued|failed|pending/i);
      expect(coordinator.get(clientScope)).toBe(sourceBefore);
      expect(coordinator.get(destinationScope)).toBe(destinationBefore);
      expect(coordinator.resolveScope(clientScope)).toBe(clientScope);

      await settleDestination();
    },
  );

  it('counts a blocked create promotion as at-risk and retries it without absorbing unsafe work', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ name: string; notes: string }>();
    const clientScope = newEntityScope('camp-a', 'npc', 'client-draft');
    const destinationScope = entityScope('camp-a', 'npc', 'sable');
    const destinationTarget = 'entity:npc:sable';
    coordinator.open(clientScope, clientScope, { name: '', notes: '' });
    coordinator.revise(clientScope, { name: 'Captain Sable', notes: 'Create revision' });

    const create = coordinator.requestSave(clientScope, writer.write);
    required(writer.attempts[0], 'the create attempt').completion.resolve({
      name: 'Captain Sable',
      notes: 'Canonical create revision',
    });
    await create;
    coordinator.open(destinationScope, destinationTarget, {
      name: 'Captain Sable',
      notes: 'Canonical create revision',
    });
    coordinator.revise(destinationScope, {
      name: 'Captain Sable',
      notes: 'Independent destination revision',
    });

    expect(coordinator.atRiskCount()).toBe(1);
    const sourceBefore = coordinator.get(clientScope);
    const destinationBefore = coordinator.get(destinationScope);
    const blocked = coordinator.tryRemapAfterCreate(
      clientScope,
      destinationScope,
      destinationTarget,
    );

    expect(blocked).toEqual({ outcome: 'blocked', reason: 'destination-at-risk' });
    expect(coordinator.atRiskCount()).toBe(2);
    expect(coordinator.getCreatePromotionIssue(clientScope)).toMatchObject({
      sourceScope: clientScope,
      destinationScope,
      destinationTarget,
      reason: 'destination-at-risk',
    });

    const retried = coordinator.retryCreatePromotion(clientScope);

    expect(retried).toEqual({ outcome: 'blocked', reason: 'destination-at-risk' });
    expect(coordinator.get(clientScope)).toBe(sourceBefore);
    expect(coordinator.get(destinationScope)).toBe(destinationBefore);
    expect(coordinator.resolveScope(clientScope)).toBe(clientScope);
    expect(coordinator.getCreatePromotionIssue(clientScope)).toMatchObject({
      sourceScope: clientScope,
      destinationScope,
      destinationTarget,
      reason: 'destination-at-risk',
    });
    expect(coordinator.atRiskCount()).toBe(2);
  });

  it.each([
    ['source alias', 'source'],
    ['destination scope', 'destination'],
  ] as const)(
    'discards a remapped draft through its %s and removes only its redirect',
    async (_description, discardThrough) => {
      const coordinator = new DraftCoordinator();
      const writer = new ControlledWriter<{ name: string; notes: string }>();
      const clientScope = newEntityScope('camp-a', 'npc', 'client-draft');
      const existingScope = entityScope('camp-a', 'npc', 'sable');
      const unrelatedScope = entityScope('camp-a', 'npc', 'mira');
      const existingTarget = 'entity:npc:sable';
      coordinator.open(clientScope, clientScope, { name: '', notes: '' });
      coordinator.revise(clientScope, { name: 'Captain Sable', notes: 'Create revision' });
      coordinator.open(unrelatedScope, 'entity:npc:mira', {
        name: 'Mira',
        notes: 'Saved unrelated notes',
      });
      coordinator.revise(unrelatedScope, { name: 'Mira', notes: 'Retained unrelated draft' });

      const create = coordinator.requestSave(clientScope, writer.write);
      required(writer.attempts[0], 'the create attempt').completion.resolve({
        name: 'Captain Sable',
        notes: 'Canonical create revision',
      });
      await create;
      coordinator.remapAfterCreate(clientScope, existingScope, existingTarget);
      coordinator.revise(existingScope, {
        name: 'Captain Sable',
        notes: 'Unsaved update after create',
      });

      const selectedScope = discardThrough === 'source' ? clientScope : existingScope;
      expect(coordinator.discard(selectedScope)).toBe('discarded');

      expect(coordinator.resolveScope(clientScope)).toBe(clientScope);
      expect(coordinator.resolveScope(existingScope)).toBe(existingScope);
      expect(coordinator.get(existingScope)).toMatchObject({
        baseline: { name: 'Captain Sable', notes: 'Canonical create revision' },
        value: { name: 'Captain Sable', notes: 'Canonical create revision' },
      });
      expect(
        statusOf(required(coordinator.get(existingScope), 'the discarded remapped draft')),
      ).toBe('saved');
      expect(coordinator.get(unrelatedScope)).toMatchObject({
        value: { name: 'Mira', notes: 'Retained unrelated draft' },
      });
      expect(statusOf(required(coordinator.get(unrelatedScope), 'the unrelated draft'))).toBe(
        'pending',
      );
    },
  );

  it('clears remap redirects when all drafts are discarded', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ name: string; notes: string }>();
    const clientScope = newEntityScope('camp-a', 'npc', 'client-draft');
    const existingScope = entityScope('camp-a', 'npc', 'sable');
    const existingTarget = 'entity:npc:sable';
    coordinator.open(clientScope, clientScope, { name: '', notes: '' });
    coordinator.revise(clientScope, { name: 'Captain Sable', notes: 'Create revision' });

    const create = coordinator.requestSave(clientScope, writer.write);
    required(writer.attempts[0], 'the create attempt').completion.resolve({
      name: 'Captain Sable',
      notes: 'Canonical create revision',
    });
    await create;
    coordinator.remapAfterCreate(clientScope, existingScope, existingTarget);
    expect(coordinator.resolveScope(clientScope)).toBe(existingScope);

    expect(coordinator.discardAll()).toBe('discarded');

    expect(coordinator.resolveScope(clientScope)).toBe(clientScope);
    expect(coordinator.resolveScope(existingScope)).toBe(existingScope);
    expect(coordinator.get(existingScope)).toBeUndefined();
  });

  it('retains a pending or failed client draft when no applicable create acknowledgment exists', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ name: string; notes: string }>();
    const clientScope = newEntityScope('camp-a', 'npc', 'client-draft');
    const existingScope = entityScope('camp-a', 'npc', 'sable');
    const existingTarget = 'entity:npc:sable';
    coordinator.open(clientScope, clientScope, { name: '', notes: '' });
    coordinator.revise(clientScope, { name: 'Captain Sable', notes: 'Retain me' });

    const pending = coordinator.get(clientScope);
    expect(() => coordinator.remapAfterCreate(clientScope, existingScope, existingTarget)).toThrow(
      /acknowledg|pending|settled/i,
    );
    expect(coordinator.get(clientScope)).toBe(pending);
    expect(coordinator.get(existingScope)).toBeUndefined();

    const create = coordinator.requestSave(clientScope, writer.write);
    required(writer.attempts[0], 'the failing create attempt').completion.reject(
      new Error('create unavailable'),
    );
    await create;
    const failed = coordinator.get(clientScope);

    expect(() => coordinator.remapAfterCreate(clientScope, existingScope, existingTarget)).toThrow(
      /failed|acknowledg/i,
    );
    expect(coordinator.get(clientScope)).toBe(failed);
    expect(coordinator.get(existingScope)).toBeUndefined();
    expect(statusOf(required(failed, 'the retained failed create'))).toBe('failed');
  });

  it('rejects a missing or actively saving source without creating the destination', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ name: string; notes: string }>();
    const missingScope = newEntityScope('camp-a', 'npc', 'missing-client-draft');
    const clientScope = newEntityScope('camp-a', 'npc', 'client-draft');
    const existingScope = entityScope('camp-a', 'npc', 'sable');
    const existingTarget = 'entity:npc:sable';

    expect(() => coordinator.remapAfterCreate(missingScope, existingScope, existingTarget)).toThrow(
      /missing|not open/i,
    );
    expect(coordinator.get(existingScope)).toBeUndefined();

    coordinator.open(clientScope, clientScope, { name: '', notes: '' });
    coordinator.revise(clientScope, { name: 'Captain Sable', notes: 'Creating' });
    const create = coordinator.requestSave(clientScope, writer.write);
    const active = coordinator.get(clientScope);

    expect(() => coordinator.remapAfterCreate(clientScope, existingScope, existingTarget)).toThrow(
      /active|saving|settled/i,
    );
    expect(coordinator.get(clientScope)).toBe(active);
    expect(coordinator.get(existingScope)).toBeUndefined();

    required(writer.attempts[0], 'the active create attempt').completion.resolve({
      name: 'Captain Sable',
      notes: 'Creating',
    });
    await create;
  });

  it('rejects a queued source without changing either scope', async () => {
    const coordinator = new DraftCoordinator();
    const blockerWriter = new ControlledWriter<{ name: string; notes: string }>();
    const queuedWriter = new ControlledWriter<{ name: string; notes: string }>();
    const clientScope = newEntityScope('camp-a', 'npc', 'client-draft');
    const blockerScope = 'create-blocker:camp-a:npc';
    const existingScope = entityScope('camp-a', 'npc', 'sable');
    const existingTarget = 'entity:npc:sable';
    coordinator.open(blockerScope, clientScope, { name: 'Blocker', notes: '' });
    coordinator.open(clientScope, clientScope, { name: '', notes: '' });
    coordinator.revise(blockerScope, { name: 'Blocker', notes: 'Saving' });
    coordinator.revise(clientScope, { name: 'Captain Sable', notes: 'Queued create' });

    const blockerSave = coordinator.requestSave(blockerScope, blockerWriter.write);
    const queuedSave = coordinator.requestSave(clientScope, queuedWriter.write);
    const queued = coordinator.get(clientScope);

    expect(queuedWriter.attempts).toHaveLength(0);
    expect(() => coordinator.remapAfterCreate(clientScope, existingScope, existingTarget)).toThrow(
      /queued|settled|acknowledg/i,
    );
    expect(coordinator.get(clientScope)).toBe(queued);
    expect(coordinator.get(existingScope)).toBeUndefined();

    required(blockerWriter.attempts[0], 'the blocking write').completion.resolve({
      name: 'Blocker',
      notes: 'Saving',
    });
    await vi.waitFor(() => expect(queuedWriter.attempts).toHaveLength(1));
    required(queuedWriter.attempts[0], 'the formerly queued create').completion.resolve({
      name: 'Captain Sable',
      notes: 'Queued create',
    });
    await Promise.all([blockerSave, queuedSave]);
  });

  it('rejects occupied destination scopes and targets atomically', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ name: string; notes: string }>();
    const clientScope = newEntityScope('camp-a', 'npc', 'client-draft');
    const occupiedScope = entityScope('camp-a', 'npc', 'occupied');
    const freeScope = entityScope('camp-a', 'npc', 'sable');
    const occupiedTarget = 'entity:npc:occupied';
    coordinator.open(clientScope, clientScope, { name: '', notes: '' });
    coordinator.revise(clientScope, { name: 'Captain Sable', notes: 'Create revision' });
    const create = coordinator.requestSave(clientScope, writer.write);
    required(writer.attempts[0], 'the create attempt').completion.resolve({
      name: 'Captain Sable',
      notes: 'Canonical create revision',
    });
    await create;
    coordinator.open(occupiedScope, occupiedTarget, { name: 'Occupied', notes: 'Keep me' });

    const sourceBefore = coordinator.get(clientScope);
    const occupiedBefore = coordinator.get(occupiedScope);
    expect(coordinator.resolveScope(clientScope)).toBe(clientScope);
    expect(() =>
      coordinator.remapAfterCreate(clientScope, occupiedScope, 'entity:npc:sable'),
    ).toThrow(/occupied|destination|scope/i);
    expect(coordinator.resolveScope(clientScope)).toBe(clientScope);
    expect(coordinator.get(clientScope)).toBe(sourceBefore);
    expect(coordinator.get(occupiedScope)).toBe(occupiedBefore);

    expect(() => coordinator.remapAfterCreate(clientScope, freeScope, occupiedTarget)).toThrow(
      /occupied|destination|target/i,
    );
    expect(coordinator.resolveScope(clientScope)).toBe(clientScope);
    expect(coordinator.get(clientScope)).toBe(sourceBefore);
    expect(coordinator.get(freeScope)).toBeUndefined();
    expect(coordinator.get(occupiedScope)).toBe(occupiedBefore);
  });

  it('rejects a second remap that would create a redirect chain or cycle', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ name: string; notes: string }>();
    const clientScope = newEntityScope('camp-a', 'npc', 'client-draft');
    const existingScope = entityScope('camp-a', 'npc', 'sable');
    const chainedScope = entityScope('camp-a', 'npc', 'sable-copy');
    const existingTarget = 'entity:npc:sable';
    coordinator.open(clientScope, clientScope, { name: '', notes: '' });
    coordinator.revise(clientScope, { name: 'Captain Sable', notes: 'Create revision' });

    const create = coordinator.requestSave(clientScope, writer.write);
    required(writer.attempts[0], 'the create attempt').completion.resolve({
      name: 'Captain Sable',
      notes: 'Canonical create revision',
    });
    await create;
    const remapped = coordinator.remapAfterCreate(clientScope, existingScope, existingTarget);

    expect(() =>
      coordinator.remapAfterCreate(existingScope, chainedScope, 'entity:npc:sable-copy'),
    ).toThrow(/create|redirect|remap|source/i);
    expect(() =>
      coordinator.remapAfterCreate(existingScope, clientScope, 'entity:npc:client-draft'),
    ).toThrow(/create|redirect|remap|source|cycle/i);

    expect(coordinator.resolveScope(clientScope)).toBe(existingScope);
    expect(coordinator.resolveScope(existingScope)).toBe(existingScope);
    expect(coordinator.get(existingScope)).toBe(remapped);
    expect(coordinator.get(chainedScope)).toBeUndefined();
    expect(coordinator.get(clientScope)).toBeUndefined();
  });
});

describe('DraftCoordinator save lanes', () => {
  it('serializes one active write per target and coalesces three requests to first plus newest', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ notes: string }>();
    const scope = entityScope('camp-a', 'npc', 'mira');
    coordinator.open(scope, 'entity:npc:mira', { notes: 'saved' });

    coordinator.revise(scope, { notes: 'first' });
    const firstSave = coordinator.requestSave(scope, writer.write);
    coordinator.revise(scope, { notes: 'second' });
    const secondSave = coordinator.requestSave(scope, writer.write);
    coordinator.revise(scope, { notes: 'newest' });
    const newestSave = coordinator.requestSave(scope, writer.write);

    expect(writer.activeWrites).toBe(1);
    expect(writer.attempts.map(({ value }) => value)).toEqual([{ notes: 'first' }]);
    expect(statusOf(required(coordinator.get(scope), 'the saving draft'))).toBe('saving');

    required(writer.attempts[0], 'the first write attempt').completion.resolve({ notes: 'first' });
    await vi.waitFor(() => expect(writer.attempts).toHaveLength(2));

    expect(writer.activeWrites).toBe(1);
    expect(required(writer.attempts[1], 'the coalesced write attempt').value).toEqual({
      notes: 'newest',
    });
    expect(coordinator.get<{ notes: string }>(scope)).toMatchObject({
      baseline: { notes: 'first' },
      value: { notes: 'newest' },
    });

    required(writer.attempts[1], 'the coalesced write attempt').completion.resolve({
      notes: 'newest',
    });
    await Promise.all([firstSave, secondSave, newestSave]);

    expect(writer.maxConcurrentWrites).toBe(1);
    expect(writer.persisted).toEqual([{ notes: 'first' }, { notes: 'newest' }]);
    expect(coordinator.get<{ notes: string }>(scope)).toMatchObject({
      baseline: { notes: 'newest' },
      value: { notes: 'newest' },
      error: null,
    });
    expect(statusOf(required(coordinator.get(scope), 'the acknowledged draft'))).toBe('saved');
  });

  it('adopts an applicable canonical acknowledgment when no newer revision exists', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ notes: string }>();
    const scope = entityScope('camp-a', 'npc', 'mira');
    coordinator.open(scope, 'entity:npc:mira', { notes: 'saved' });
    coordinator.revise(scope, { notes: 'locally entered value' });

    const save = coordinator.requestSave(scope, writer.write);
    required(writer.attempts[0], 'the canonicalized write').completion.resolve({
      notes: 'canonical saved value',
    });
    await save;

    expect(coordinator.get<{ notes: string }>(scope)).toMatchObject({
      baseline: { notes: 'canonical saved value' },
      value: { notes: 'canonical saved value' },
      revision: 1,
      acknowledgedRevision: 1,
    });
    expect(statusOf(required(coordinator.get(scope), 'the canonicalized draft'))).toBe('saved');
  });

  it('captures immutable payloads and applies completions only to their original scopes', async () => {
    const coordinator = new DraftCoordinator();
    const firstWriter = new ControlledWriter<{ notes: string }>();
    const queuedWriter = new ControlledWriter<{ notes: string }>();
    const target = 'rule:initiative';
    const campaignA = 'rule:camp-a:world-guide:initiative';
    const campaignB = 'rule:camp-b:world-guide:initiative';
    const firstValue = { notes: 'Campaign A note' };

    coordinator.open(campaignA, target, { notes: 'A saved' });
    coordinator.open(campaignB, target, { notes: 'B saved' });
    coordinator.revise(campaignA, firstValue);
    const firstSave = coordinator.requestSave(campaignA, firstWriter.write);
    coordinator.revise(campaignB, { notes: 'Campaign B note' });
    const queuedSave = coordinator.requestSave(campaignB, queuedWriter.write);
    firstValue.notes = 'mutated outside coordinator';

    expect(required(firstWriter.attempts[0], 'the Campaign A write attempt').value).toEqual({
      notes: 'Campaign A note',
    });
    expect(queuedWriter.attempts).toHaveLength(0);

    required(firstWriter.attempts[0], 'the Campaign A write attempt').completion.resolve({
      notes: 'Campaign A note',
    });
    await vi.waitFor(() => expect(queuedWriter.attempts).toHaveLength(1));

    expect(coordinator.get<{ notes: string }>(campaignA)).toMatchObject({
      baseline: { notes: 'Campaign A note' },
      value: { notes: 'Campaign A note' },
    });
    expect(statusOf(required(coordinator.get(campaignA), 'the Campaign A draft'))).toBe('saved');
    expect(required(queuedWriter.attempts[0], 'the Campaign B write attempt').value).toEqual({
      notes: 'Campaign B note',
    });

    required(queuedWriter.attempts[0], 'the Campaign B write attempt').completion.resolve({
      notes: 'Campaign B note',
    });
    await Promise.all([firstSave, queuedSave]);

    expect(firstWriter.maxConcurrentWrites).toBe(1);
    expect(queuedWriter.maxConcurrentWrites).toBe(1);
    expect(coordinator.get<{ notes: string }>(campaignB)).toMatchObject({
      baseline: { notes: 'Campaign B note' },
      value: { notes: 'Campaign B note' },
    });
    expect(statusOf(required(coordinator.get(campaignB), 'the Campaign B draft'))).toBe('saved');
  });

  it('preserves queued intents from distinct scopes sharing a target while coalescing each scope tail', async () => {
    const coordinator = new DraftCoordinator();
    const firstAWriter = new ControlledWriter<{ notes: string }>();
    const campaignBWriter = new ControlledWriter<{ notes: string }>();
    const latestAWriter = new ControlledWriter<{ notes: string }>();
    const target = 'rule:initiative';
    const campaignA = 'rule:camp-a:world-guide:initiative';
    const campaignB = 'rule:camp-b:world-guide:initiative';
    coordinator.open(campaignA, target, { notes: 'Shared saved note' });
    coordinator.open(campaignB, target, { notes: 'Shared saved note' });

    coordinator.revise(campaignA, { notes: 'Campaign A first request' });
    const firstA = coordinator.requestSave(campaignA, firstAWriter.write);
    coordinator.revise(campaignB, { notes: 'Campaign B queued request' });
    const queuedB = coordinator.requestSave(campaignB, campaignBWriter.write);
    coordinator.revise(campaignA, { notes: 'Campaign A newest request' });
    const latestA = coordinator.requestSave(campaignA, latestAWriter.write);

    expect(firstAWriter.attempts).toHaveLength(1);
    expect(campaignBWriter.attempts).toHaveLength(0);
    expect(latestAWriter.attempts).toHaveLength(0);

    required(firstAWriter.attempts[0], 'Campaign A active request').completion.resolve({
      notes: 'Campaign A first request',
    });
    await vi.waitFor(() => expect(campaignBWriter.attempts).toHaveLength(1));
    expect(required(campaignBWriter.attempts[0], 'Campaign B preserved request').value).toEqual({
      notes: 'Campaign B queued request',
    });
    expect(latestAWriter.attempts).toHaveLength(0);

    required(campaignBWriter.attempts[0], 'Campaign B preserved request').completion.resolve({
      notes: 'Campaign B queued request',
    });
    await vi.waitFor(() => expect(latestAWriter.attempts).toHaveLength(1));
    expect(required(latestAWriter.attempts[0], 'Campaign A newest request').value).toEqual({
      notes: 'Campaign A newest request',
    });

    required(latestAWriter.attempts[0], 'Campaign A newest request').completion.resolve({
      notes: 'Campaign A newest request',
    });
    await Promise.all([firstA, queuedB, latestA]);

    expect(statusOf(required(coordinator.get(campaignA), 'Campaign A final draft'))).toBe('saved');
    expect(statusOf(required(coordinator.get(campaignB), 'Campaign B final draft'))).toBe('saved');
  });

  it('preserves newer edits when an earlier save completes without a queued request', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ title: string }>();
    const scope = 'session:camp-a:session-7';
    coordinator.open(scope, 'session:session-7', { title: 'Saved title' });
    coordinator.revise(scope, { title: 'Saving title' });

    const save = coordinator.requestSave(scope, writer.write);
    coordinator.revise(scope, { title: 'Newer local title' });
    required(writer.attempts[0], 'the earlier title write').completion.resolve({
      title: 'Saving title',
    });
    await save;

    expect(coordinator.get<{ title: string }>(scope)).toMatchObject({
      baseline: { title: 'Saving title' },
      value: { title: 'Newer local title' },
    });
    expect(statusOf(required(coordinator.get(scope), 'the newer session draft'))).toBe('pending');
  });

  it('keeps a coincident newer revision at risk after an older save is acknowledged', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ title: string }>();
    const scope = 'session:camp-a:session-7';
    coordinator.open(scope, 'session:session-7', { title: 'Saved title' });
    coordinator.revise(scope, { title: 'First revision' });

    const firstSave = coordinator.requestSave(scope, writer.write);
    coordinator.revise(scope, { title: 'Canonical second revision' });
    required(writer.attempts[0], 'the first revision write').completion.resolve({
      title: 'Canonical second revision',
    });
    await firstSave;

    expect(statusOf(required(coordinator.get(scope), 'the coincident newer revision'))).toBe(
      'pending',
    );
    expect(coordinator.atRiskCount()).toBe(1);
    expect(coordinator.get<{ title: string }>(scope)).toMatchObject({
      baseline: { title: 'Canonical second revision' },
      value: { title: 'Canonical second revision' },
      revision: 2,
      acknowledgedRevision: 1,
      unacknowledgedRevision: 2,
      inFlight: null,
    });
  });

  it('clears coincident risk when a later deliberate edit returns to the known baseline', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ title: string }>();
    const scope = 'session:camp-a:session-7';
    coordinator.open(scope, 'session:session-7', { title: 'Saved title' });
    coordinator.revise(scope, { title: 'First revision' });

    const firstSave = coordinator.requestSave(scope, writer.write);
    coordinator.revise(scope, { title: 'Canonical second revision' });
    required(writer.attempts[0], 'the first revision write').completion.resolve({
      title: 'Canonical second revision',
    });
    await firstSave;

    coordinator.revise(scope, { title: 'Deliberate third revision' });
    coordinator.revise(scope, { title: 'Canonical second revision' });

    expect(writer.attempts).toHaveLength(1);
    expect(coordinator.get<{ title: string }>(scope)).toMatchObject({
      baseline: { title: 'Canonical second revision' },
      value: { title: 'Canonical second revision' },
      revision: 4,
      acknowledgedRevision: 1,
      inFlight: null,
    });
    expect(statusOf(required(coordinator.get(scope), 'the deliberately reverted draft'))).toBe(
      'saved',
    );
    expect(coordinator.atRiskCount()).toBe(0);
  });

  it('retains a failed value and retries the latest revision against the same target', async () => {
    const coordinator = new DraftCoordinator();
    const failingWriter = new ControlledWriter<{ notes: string }>();
    const retryWriter = new ControlledWriter<{ notes: string }>();
    const scope = entityScope('camp-a', 'npc', 'mira');
    coordinator.open(scope, 'entity:npc:mira', { notes: 'saved' });
    coordinator.revise(scope, { notes: 'first attempt' });

    const firstSave = coordinator.requestSave(scope, failingWriter.write);
    coordinator.revise(scope, { notes: 'latest retained value' });
    required(failingWriter.attempts[0], 'the failing write attempt').completion.reject(
      new Error('database locked'),
    );
    await firstSave;

    expect(coordinator.get<{ notes: string }>(scope)).toMatchObject({
      target: 'entity:npc:mira',
      baseline: { notes: 'saved' },
      value: { notes: 'latest retained value' },
      error: 'database locked',
    });
    expect(statusOf(required(coordinator.get(scope), 'the failed draft'))).toBe('failed');

    const retry = coordinator.requestSave(scope, retryWriter.write);
    expect(required(retryWriter.attempts[0], 'the retry attempt').value).toEqual({
      notes: 'latest retained value',
    });
    required(retryWriter.attempts[0], 'the retry attempt').completion.resolve({
      notes: 'latest retained value',
    });
    await retry;

    expect(retryWriter.persisted).toEqual([{ notes: 'latest retained value' }]);
    expect(coordinator.get(scope)?.error).toBeNull();
    expect(statusOf(required(coordinator.get(scope), 'the retried draft'))).toBe('saved');
  });

  it('turns an invalid canonical response into a retryable failure without losing newer edits', async () => {
    const coordinator = new DraftCoordinator();
    const invalidCanonical = deferred<{ notes: string }>();
    const retryWriter = new ControlledWriter<{ notes: string }>();
    const scope = entityScope('camp-a', 'npc', 'mira');
    coordinator.open(scope, 'entity:npc:mira', { notes: 'saved' });
    coordinator.revise(scope, { notes: 'first attempt' });

    const save = coordinator.requestSave(scope, () => invalidCanonical.promise);
    coordinator.revise(scope, { notes: 'latest retained value' });
    const cyclicCanonical: { notes: string; self?: unknown } = {
      notes: 'invalid canonical value',
    };
    cyclicCanonical.self = cyclicCanonical;
    invalidCanonical.resolve(cyclicCanonical);

    await expect(save).resolves.toBeUndefined();
    expect(coordinator.get<{ notes: string }>(scope)).toMatchObject({
      baseline: { notes: 'saved' },
      value: { notes: 'latest retained value' },
      inFlight: null,
      error: expect.stringMatching(/cycles/i),
    });
    expect(statusOf(required(coordinator.get(scope), 'the invalid-response draft'))).toBe('failed');

    const retry = coordinator.requestSave(scope, retryWriter.write);
    expect(required(retryWriter.attempts[0], 'the retry after invalid response').value).toEqual({
      notes: 'latest retained value',
    });
    required(retryWriter.attempts[0], 'the retry after invalid response').completion.resolve({
      notes: 'latest retained value',
    });
    await retry;

    expect(coordinator.get<{ notes: string }>(scope)).toMatchObject({
      baseline: { notes: 'latest retained value' },
      value: { notes: 'latest retained value' },
      error: null,
    });
    expect(statusOf(required(coordinator.get(scope), 'the recovered draft'))).toBe('saved');
  });

  it('does not cancel a pending save when navigation leaves no active subscriber', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ notes: string }>();
    const scope = entityScope('camp-a', 'npc', 'mira');
    coordinator.open(scope, 'entity:npc:mira', { notes: 'saved' });
    coordinator.revise(scope, { notes: 'save while elsewhere' });

    const save = coordinator.requestSave(scope, writer.write);
    await Promise.resolve();

    expect(coordinator.get<{ notes: string }>(scope)?.value.notes).toBe('save while elsewhere');
    expect(statusOf(required(coordinator.get(scope), 'the off-screen saving draft'))).toBe(
      'saving',
    );

    required(writer.attempts[0], 'the off-screen write attempt').completion.resolve({
      notes: 'save while elsewhere',
    });
    await save;

    expect(writer.persisted).toEqual([{ notes: 'save while elsewhere' }]);
    expect(statusOf(required(coordinator.get(scope), 'the off-screen acknowledged draft'))).toBe(
      'saved',
    );
  });
});

describe('DraftCoordinator discard and close risk', () => {
  it('discards only the selected draft while preserving unrelated drafts', () => {
    const coordinator = new DraftCoordinator();
    const mira = entityScope('camp-a', 'npc', 'mira');
    const torvin = entityScope('camp-a', 'npc', 'torvin');
    const oracle = oracleScope('camp-a');
    coordinator.open(mira, 'entity:npc:mira', { notes: 'Mira saved' });
    coordinator.open(torvin, 'entity:npc:torvin', { notes: 'Torvin saved' });
    coordinator.open(oracle, null, '');
    coordinator.revise(mira, { notes: 'Mira changed' });
    coordinator.revise(torvin, { notes: 'Torvin changed' });
    coordinator.revise(oracle, 'Unsent question');

    coordinator.discard(mira);

    expect(coordinator.get<{ notes: string }>(mira)?.value).toEqual({ notes: 'Mira saved' });
    expect(coordinator.get<{ notes: string }>(torvin)?.value).toEqual({
      notes: 'Torvin changed',
    });
    expect(coordinator.get<string>(oracle)?.value).toBe('Unsent question');
    expect(coordinator.atRiskCount()).toBe(2);
  });

  it('blocks discard for the actively saved scope but permits an unrelated scope', async () => {
    const coordinator = new DraftCoordinator();
    const writer = new ControlledWriter<{ notes: string }>();
    const active = entityScope('camp-a', 'npc', 'mira');
    const unrelated = entityScope('camp-a', 'npc', 'torvin');
    coordinator.open(active, 'entity:npc:mira', { notes: 'Mira saved' });
    coordinator.open(unrelated, 'entity:npc:torvin', { notes: 'Torvin saved' });
    coordinator.revise(active, { notes: 'Mira saving' });
    coordinator.revise(unrelated, { notes: 'Torvin changed' });

    const save = coordinator.requestSave(active, writer.write);
    coordinator.revise(active, { notes: 'Mira newer local edit' });

    expect(coordinator.canDiscard(active)).toBe(false);
    expect(coordinator.discard(active)).toBe('blocked-active-save');
    expect(coordinator.get<{ notes: string }>(active)).toMatchObject({
      baseline: { notes: 'Mira saved' },
      value: { notes: 'Mira newer local edit' },
    });
    expect(statusOf(required(coordinator.get(active), 'the active draft'))).toBe('saving');

    expect(coordinator.canDiscard(unrelated)).toBe(true);
    expect(coordinator.discard(unrelated)).toBe('discarded');
    expect(coordinator.get<{ notes: string }>(unrelated)?.value).toEqual({
      notes: 'Torvin saved',
    });

    required(writer.attempts[0], 'the active write').completion.resolve({ notes: 'Mira saving' });
    await save;

    expect(writer.persisted).toEqual([{ notes: 'Mira saving' }]);
    expect(coordinator.get<{ notes: string }>(active)).toMatchObject({
      baseline: { notes: 'Mira saving' },
      value: { notes: 'Mira newer local edit' },
    });
    expect(coordinator.canDiscard(active)).toBe(true);
    expect(coordinator.discard(active)).toBe('discarded');
    expect(coordinator.get<{ notes: string }>(active)?.value).toEqual({ notes: 'Mira saving' });
    expect(statusOf(required(coordinator.get(active), 'the discarded newer revision'))).toBe(
      'saved',
    );
  });

  it('cancels and discards a queued scope without disturbing another scope active on its target', async () => {
    const coordinator = new DraftCoordinator();
    const activeWriter = new ControlledWriter<{ notes: string }>();
    const queuedWriter = new ControlledWriter<{ notes: string }>();
    const target = 'rule:initiative';
    const active = 'rule:camp-a:world-guide:initiative';
    const queued = 'rule:camp-b:world-guide:initiative';
    coordinator.open(active, target, { notes: 'A saved' });
    coordinator.open(queued, target, { notes: 'B saved' });
    coordinator.revise(active, { notes: 'A saving' });
    coordinator.revise(queued, { notes: 'B queued' });

    const activeSave = coordinator.requestSave(active, activeWriter.write);
    const queuedSave = coordinator.requestSave(queued, queuedWriter.write);

    expect(coordinator.canDiscard(queued)).toBe(true);
    expect(coordinator.discard(queued)).toBe('discarded');
    expect(coordinator.get<{ notes: string }>(queued)?.value).toEqual({ notes: 'B saved' });
    expect(queuedWriter.attempts).toHaveLength(0);

    required(activeWriter.attempts[0], 'the active target write').completion.resolve({
      notes: 'A saving',
    });
    await Promise.all([activeSave, queuedSave]);

    expect(activeWriter.persisted).toEqual([{ notes: 'A saving' }]);
    expect(queuedWriter.persisted).toEqual([]);
    expect(statusOf(required(coordinator.get(queued), 'the discarded queued draft'))).toBe('saved');
  });

  it('removes only a deleted scope after blocking its active write and settling its queued intent', async () => {
    type DeleteCleanupResult = 'removed' | 'blocked-active-save' | 'missing';
    type DeleteAwareCoordinator = DraftCoordinator & {
      removeAfterDelete(scope: string): DeleteCleanupResult;
    };

    const coordinator = new DraftCoordinator();
    const deleteAwareCoordinator = coordinator as DeleteAwareCoordinator;
    const deletedWriter = new ControlledWriter<{ title: string }>();
    const sameTargetWriter = new ControlledWriter<{ title: string }>();
    const queuedDeletedWriter = new ControlledWriter<{ title: string }>();
    const deletedScope = 'session:camp-a:session-7';
    const sameTargetScope = 'session:camp-b:session-7';
    const unrelatedScope = 'session:camp-a:session-8';
    const deletedTarget = 'session:session-7';
    coordinator.open(deletedScope, deletedTarget, { title: 'Saved seven' });
    coordinator.open(sameTargetScope, deletedTarget, { title: 'Other campaign seven' });
    coordinator.open(unrelatedScope, 'session:session-8', { title: 'Saved eight' });
    coordinator.revise(unrelatedScope, { title: 'Unrelated pending eight' });

    coordinator.revise(deletedScope, { title: 'Seven saving' });
    const activeDeletedSave = coordinator.requestSave(deletedScope, deletedWriter.write);

    expect(deleteAwareCoordinator.removeAfterDelete(deletedScope)).toBe('blocked-active-save');
    expect(coordinator.get<{ title: string }>(deletedScope)?.value).toEqual({
      title: 'Seven saving',
    });
    expect(coordinator.get<{ title: string }>(unrelatedScope)?.value).toEqual({
      title: 'Unrelated pending eight',
    });

    required(deletedWriter.attempts[0], 'the active deleted-scope write').completion.resolve({
      title: 'Seven saving',
    });
    await activeDeletedSave;

    coordinator.revise(sameTargetScope, { title: 'Other campaign saving' });
    const sameTargetSave = coordinator.requestSave(sameTargetScope, sameTargetWriter.write);
    coordinator.revise(deletedScope, { title: 'Seven queued after acknowledgment' });
    let queuedDeletedSaveSettled = false;
    const queuedDeletedSave = coordinator
      .requestSave(deletedScope, queuedDeletedWriter.write)
      .then(() => {
        queuedDeletedSaveSettled = true;
      });

    expect(queuedDeletedWriter.attempts).toHaveLength(0);
    expect(coordinator.atRiskCount()).toBe(3);
    expect(deleteAwareCoordinator.removeAfterDelete(deletedScope)).toBe('removed');
    await queuedDeletedSave;

    expect(queuedDeletedSaveSettled).toBe(true);
    expect(coordinator.get(deletedScope)).toBeUndefined();
    expect(queuedDeletedWriter.attempts).toHaveLength(0);
    expect(coordinator.get<{ title: string }>(sameTargetScope)?.value).toEqual({
      title: 'Other campaign saving',
    });
    expect(coordinator.get<{ title: string }>(unrelatedScope)?.value).toEqual({
      title: 'Unrelated pending eight',
    });
    expect(coordinator.atRiskCount()).toBe(2);

    required(sameTargetWriter.attempts[0], 'the preserved same-target write').completion.resolve({
      title: 'Other campaign saving',
    });
    await sameTargetSave;

    expect(sameTargetWriter.persisted).toEqual([{ title: 'Other campaign saving' }]);
    expect(deletedWriter.persisted).toEqual([{ title: 'Seven saving' }]);
    expect(queuedDeletedWriter.persisted).toEqual([]);
    expect(coordinator.get(deletedScope)).toBeUndefined();
    expect(coordinator.get<{ title: string }>(unrelatedScope)?.value).toEqual({
      title: 'Unrelated pending eight',
    });
  });

  it('reports and refuses discardAll while any write is active without hiding its outcome', async () => {
    const coordinator = new DraftCoordinator();
    const savingWriter = new ControlledWriter<string>();
    const failedWriter = new ControlledWriter<string>();
    coordinator.open('oracle:camp-a', null, '');
    coordinator.open('session:camp-a:s1', 'session:s1', 'saved session');
    coordinator.open('rule:camp-a:book:r1', 'rule:r1', 'saved rule');
    coordinator.revise('oracle:camp-a', 'Unsent question');
    coordinator.revise('session:camp-a:s1', 'Saving session');
    coordinator.revise('rule:camp-a:book:r1', 'Failed rule note');

    const saving = coordinator.requestSave('session:camp-a:s1', savingWriter.write);
    const failing = coordinator.requestSave('rule:camp-a:book:r1', failedWriter.write);
    required(failedWriter.attempts[0], 'the unavailable-target write').completion.reject(
      new Error('target unavailable'),
    );
    await failing;

    expect(coordinator.atRiskCount()).toBe(3);

    expect(coordinator.hasActiveWrites()).toBe(true);
    expect(coordinator.canDiscardAll()).toBe(false);
    expect(coordinator.discardAll()).toBe('blocked-active-save');

    expect(coordinator.atRiskCount()).toBe(3);
    expect(coordinator.get<string>('oracle:camp-a')?.value).toBe('Unsent question');
    expect(coordinator.get<string>('session:camp-a:s1')?.value).toBe('Saving session');
    expect(coordinator.get<string>('rule:camp-a:book:r1')).toMatchObject({
      value: 'Failed rule note',
      error: 'target unavailable',
    });

    required(savingWriter.attempts[0], 'the discarded in-flight write').completion.resolve(
      'Saving session',
    );
    await saving;
    expect(savingWriter.persisted).toEqual(['Saving session']);
    expect(coordinator.hasActiveWrites()).toBe(false);
    expect(coordinator.canDiscardAll()).toBe(true);
    expect(coordinator.discardAll()).toBe('discarded');
    expect(coordinator.atRiskCount()).toBe(0);
    expect(coordinator.get('session:camp-a:s1')).toBeUndefined();
  });
});
