import { describe, expect, expectTypeOf, it } from 'vitest';
import {
  acknowledge,
  beginAttempt,
  createDraft,
  discard,
  entityScope,
  isContentDirty,
  newEntityScope,
  oracleScope,
  rejectAttempt,
  revise,
  ruleScope,
  sessionScope,
  statusOf,
  type DraftValue,
} from './draft-state';

class UnsupportedDraftClass {
  constructor(readonly notes: string) {}
}

describe('draft editing identities', () => {
  it('limits the compile-time draft boundary to normalized JSON-like values', () => {
    expectTypeOf({
      notes: 'plain data',
      tags: ['safe'],
      nested: { rank: 3 },
    }).toExtend<DraftValue>();
    expectTypeOf(BigInt(1)).not.toExtend<DraftValue>();
    expectTypeOf(new Date()).not.toExtend<DraftValue>();
    expectTypeOf(new Map([['notes', 'not plain data']])).not.toExtend<DraftValue>();
    expectTypeOf(new Set(['not plain data'])).not.toExtend<DraftValue>();
    expectTypeOf(new UnsupportedDraftClass('not plain data')).not.toExtend<DraftValue>();
  });

  it('keeps Oracle drafts separate by campaign and from the no-campaign context', () => {
    expect(oracleScope('camp-a')).toBe('oracle:camp-a');
    expect(oracleScope('camp-b')).toBe('oracle:camp-b');
    expect(oracleScope(null)).toBe('oracle:no-campaign');
    expect(new Set([oracleScope('camp-a'), oracleScope('camp-b'), oracleScope(null)]).size).toBe(3);
  });

  it('includes the campaign, editor kind, and record in persistent-record scopes', () => {
    expect(entityScope('camp-a', 'npc', 'mira')).toBe('entity:camp-a:npc:mira');
    expect(entityScope('camp-a', 'location', 'mira')).toBe('entity:camp-a:location:mira');
    expect(sessionScope('camp-a', 'session-7')).toBe('session:camp-a:session-7');
    expect(ruleScope('camp-a', 'world-guide', 'initiative')).toBe(
      'rule:camp-a:world-guide:initiative',
    );
  });

  it('keeps rule drafts stable and distinct in the explicit no-campaign context', () => {
    const noCampaign = ruleScope(null, 'world-guide', 'initiative');

    expect(noCampaign).toBe('rule:no-campaign:world-guide:initiative');
    expect(ruleScope(null, 'world-guide', 'initiative')).toBe(noCampaign);
    expect(
      new Set([
        noCampaign,
        ruleScope('camp-a', 'world-guide', 'initiative'),
        ruleScope(null, 'other-collection', 'initiative'),
        ruleScope(null, 'world-guide', 'surprise'),
      ]).size,
    ).toBe(4);
  });

  it('uses the caller-owned UUID as a stable identity for an unsaved new entity', () => {
    const clientDraftId = 'fd776b47-f07b-44ac-83a8-a3d45ad70c49';

    const first = newEntityScope('camp-a', 'npc', clientDraftId);
    const reopened = newEntityScope('camp-a', 'npc', clientDraftId);

    expect(first).toBe('entity-new:camp-a:npc:fd776b47-f07b-44ac-83a8-a3d45ad70c49');
    expect(reopened).toBe(first);
    expect(newEntityScope('camp-b', 'npc', clientDraftId)).not.toBe(first);
  });
});

describe('pure draft transitions', () => {
  const scope = entityScope('camp-a', 'npc', 'mira');
  const target = 'entity:npc:mira';

  it('freezes every returned draft record shell across its lifecycle transitions', () => {
    const created = createDraft(scope, target, { notes: 'saved' });
    const revised = revise(created, { notes: 'changed' });
    const active = beginAttempt(revised, 1);
    const acknowledged = acknowledge(active.draft, active.attempt, {
      notes: 'canonical changed',
    });
    const rejectedActive = beginAttempt(revise(acknowledged, { notes: 'fails' }), 2);
    const rejected = rejectAttempt(rejectedActive.draft, rejectedActive.attempt, 'offline');
    const discarded = discard(rejected);
    const records = [
      ['createDraft', created],
      ['revise', revised],
      ['beginAttempt', active.draft],
      ['acknowledge', acknowledged],
      ['rejectAttempt', rejected],
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
            error: 'mutated outside the state machine',
          }),
        )
        .toThrow(TypeError);
      expect.soft(record, `${transition} lifecycle metadata`).toMatchObject(original);
    }
  });

  it('starts at an acknowledged saved baseline', () => {
    const draft = createDraft(scope, target, { name: 'Mira', notes: 'saved' });

    expect(draft).toMatchObject({
      scope,
      target,
      baseline: { name: 'Mira', notes: 'saved' },
      value: { name: 'Mira', notes: 'saved' },
      revision: 0,
      acknowledgedRevision: 0,
      inFlight: null,
      error: null,
    });
    expect(isContentDirty(draft)).toBe(false);
    expect(statusOf(draft)).toBe('saved');
  });

  it('keeps an ordinary return to its pre-edit baseline clean without a write', () => {
    const saved = createDraft(scope, target, { name: 'Mira', notes: 'saved' });
    const changed = revise(saved, { name: 'Mira', notes: 'changed' });
    const reverted = revise(changed, { notes: 'saved', name: 'Mira' });

    expect(changed.revision).toBe(1);
    expect(isContentDirty(changed)).toBe(true);
    expect(statusOf(changed)).toBe('pending');
    expect(reverted.revision).toBe(2);
    expect(isContentDirty(reverted)).toBe(false);
    expect(reverted).toMatchObject({ acknowledgedRevision: 0, inFlight: null });
    expect(statusOf(reverted)).toBe('saved');
  });

  it('does not create a revision for deterministically equal editor content', () => {
    const saved = createDraft(scope, target, {
      name: 'Mira',
      aliases: ['The Seer'],
      details: { notes: 'saved', level: 4 },
    });

    const unchanged = revise(saved, {
      details: { level: 4, notes: 'saved' },
      aliases: ['The Seer'],
      name: 'Mira',
    });

    expect(unchanged.revision).toBe(0);
    expect(statusOf(unchanged)).toBe('saved');
  });

  it('captures an immutable save payload with its scope, target, and revision', () => {
    const editedValue = { notes: 'first', metadata: { tags: ['seer'] } };
    const edited = revise(
      createDraft(scope, target, { notes: '', metadata: { tags: [] } }),
      editedValue,
    );

    const { draft: saving, attempt } = beginAttempt(edited, 41);
    editedValue.metadata.tags.push('mutated after capture');

    expect(attempt).toEqual({
      id: 41,
      scope,
      target,
      revision: 1,
      value: { notes: 'first', metadata: { tags: ['seer'] } },
    });
    expect(saving.inFlight).toEqual(attempt);
    expect(statusOf(saving)).toBe('saving');
  });

  it('recursively snapshots and freezes baseline input before exposing a draft', () => {
    const baselineInput = {
      notes: 'saved',
      metadata: { tags: ['seer'], flags: { private: true } },
    };

    const created = createDraft(scope, target, baselineInput);
    baselineInput.metadata.tags.push('mutated through adapter input');
    baselineInput.metadata.flags.private = false;

    expect(created.baseline).toEqual({
      notes: 'saved',
      metadata: { tags: ['seer'], flags: { private: true } },
    });
    expect(created.value).toEqual(created.baseline);
    expect(Object.isFrozen(created.baseline)).toBe(true);
    expect(Object.isFrozen(created.baseline.metadata)).toBe(true);
    expect(Object.isFrozen(created.value.metadata.tags)).toBe(true);

    expect(() => {
      (created.value.metadata.tags as string[]).push('bypass revise');
    }).toThrow(TypeError);
    expect(created.value.metadata.tags).toEqual(['seer']);
    expect(created.revision).toBe(0);
    expect(statusOf(created)).toBe('saved');
  });

  it('recursively snapshots and freezes revised input so mutation cannot bypass revision tracking', () => {
    const revisedInput = {
      notes: 'changed',
      metadata: { tags: ['oracle'], flags: { private: false } },
    };
    const created = createDraft(scope, target, {
      notes: 'saved',
      metadata: { tags: [] as string[], flags: { private: true } },
    });

    const revised = revise(created, revisedInput);
    revisedInput.metadata.tags.push('mutated after revise');
    revisedInput.metadata.flags.private = true;

    expect(revised.value).toEqual({
      notes: 'changed',
      metadata: { tags: ['oracle'], flags: { private: false } },
    });
    expect(Object.isFrozen(revised.value.metadata)).toBe(true);
    expect(Object.isFrozen(revised.value.metadata.flags)).toBe(true);
    expect(() => {
      (revised.value.metadata.flags as { private: boolean }).private = true;
    }).toThrow(TypeError);
    expect(revised.revision).toBe(1);
    expect(statusOf(revised)).toBe('pending');
  });

  it('keeps immutable newer input and canonical snapshots through an older acknowledgment', () => {
    const first = revise(createDraft(scope, target, { metadata: { tags: ['saved'] } }), {
      metadata: { tags: ['first'] },
    });
    const active = beginAttempt(first, 7);
    const newerInput = { metadata: { tags: ['newer'] } };
    const newer = revise(active.draft, newerInput);
    const canonicalInput = { metadata: { tags: ['canonical first'] } };

    newerInput.metadata.tags.push('mutated after revise');
    expect(() => {
      (active.attempt.value.metadata.tags as string[]).push('mutated through attempt');
    }).toThrow(TypeError);

    const acknowledged = acknowledge(newer, active.attempt, canonicalInput);
    canonicalInput.metadata.tags.push('mutated after acknowledgment');

    expect(acknowledged).toMatchObject({
      baseline: { metadata: { tags: ['canonical first'] } },
      value: { metadata: { tags: ['newer'] } },
      revision: 2,
      acknowledgedRevision: 1,
      unacknowledgedRevision: 2,
      inFlight: null,
    });
    expect(Object.isFrozen(acknowledged.baseline.metadata.tags)).toBe(true);
    expect(Object.isFrozen(acknowledged.value.metadata.tags)).toBe(true);
    expect(() => {
      (acknowledged.baseline.metadata.tags as string[]).push('mutated through result');
    }).toThrow(TypeError);
    expect(statusOf(acknowledged)).toBe('pending');
  });

  it.each([
    ['bigint', BigInt(1)],
    ['Date', new Date('2026-09-07T00:00:00Z')],
    ['Map', new Map([['notes', 'unsupported']])],
    ['Set', new Set(['unsupported'])],
    ['class instance', new UnsupportedDraftClass('unsupported')],
  ])('rejects a %s before it enters draft state', (_label, unsupported) => {
    expect(() => createDraft(scope, target, unsupported as never)).toThrow();
  });

  it('rejects an invalid nested revision without changing the original record or snapshots', () => {
    const original = revise(
      createDraft(scope, target, {
        notes: 'saved',
        metadata: { tags: ['seer'] },
      }),
      {
        notes: 'valid local edit',
        metadata: { tags: ['oracle'] },
      },
    );
    const originalBaseline = original.baseline;
    const originalValue = original.value;
    const originalRevision = original.revision;

    expect(() =>
      revise(original, {
        notes: 'invalid local edit',
        metadata: { tags: ['oracle'], unsupported: new Set(['hidden mutation']) },
      } as never),
    ).toThrow(TypeError);

    expect(original.baseline).toBe(originalBaseline);
    expect(original.value).toBe(originalValue);
    expect(original).toMatchObject({
      baseline: { notes: 'saved', metadata: { tags: ['seer'] } },
      value: { notes: 'valid local edit', metadata: { tags: ['oracle'] } },
      revision: originalRevision,
      unacknowledgedRevision: originalRevision,
      inFlight: null,
      error: null,
    });
    expect(statusOf(original)).toBe('pending');
  });

  it.each([
    [
      'cyclic object',
      () => {
        const cyclic: Record<string, unknown> = {};
        cyclic.self = cyclic;
        return cyclic;
      },
    ],
    [
      'sparse array',
      () => {
        const sparse: unknown[] = [];
        sparse.length = 1;
        return sparse;
      },
    ],
    ['undefined', () => undefined],
    ['function', () => () => 'unsupported'],
    ['symbol', () => Symbol('unsupported')],
  ] satisfies Array<[string, () => unknown]>)('rejects runtime %s values', (_label, value) => {
    expect(() => createDraft(scope, target, value() as never)).toThrow(TypeError);
  });

  it.each([
    ['NaN', Number.NaN],
    ['positive Infinity', Number.POSITIVE_INFINITY],
    ['negative Infinity', Number.NEGATIVE_INFINITY],
  ])('rejects the non-finite number %s', (_label, value) => {
    expect(() => createDraft(scope, target, value as never)).toThrow(/finite/);
  });

  it('marks only the applicable acknowledged revision as saved', () => {
    const edited = revise(createDraft(scope, target, { notes: 'saved' }), { notes: 'new' });
    const { draft: saving, attempt } = beginAttempt(edited, 1);

    const acknowledged = acknowledge(saving, attempt, { notes: 'canonical new' });

    expect(acknowledged).toMatchObject({
      value: { notes: 'canonical new' },
      baseline: { notes: 'canonical new' },
      revision: 1,
      acknowledgedRevision: 1,
      inFlight: null,
      error: null,
    });
    expect(statusOf(acknowledged)).toBe('saved');
  });

  it('allows an older acknowledgment to advance the baseline without erasing newer edits', () => {
    const d0 = createDraft(scope, target, { notes: 'saved' });
    const d1 = revise(d0, { notes: 'new' });
    const a1 = beginAttempt(d1, 1);
    const d2 = revise(a1.draft, { notes: 'newer' });

    const acknowledged = acknowledge(d2, a1.attempt, { notes: 'new' });

    expect(acknowledged).toMatchObject({
      value: { notes: 'newer' },
      baseline: { notes: 'new' },
      revision: 2,
      acknowledgedRevision: 1,
      inFlight: null,
    });
    expect(statusOf(acknowledged)).toBe('pending');
  });

  it('keeps a newer revision pending when an older canonical acknowledgment equals it', () => {
    const d0 = createDraft(scope, target, { notes: 'saved' });
    const d1 = revise(d0, { notes: 'first revision' });
    const a1 = beginAttempt(d1, 1);
    const d2 = revise(a1.draft, { notes: 'canonical second revision' });

    const coincident = acknowledge(d2, a1.attempt, {
      notes: 'canonical second revision',
    });

    expect(isContentDirty(coincident)).toBe(false);
    expect(statusOf(coincident)).toBe('pending');
    expect(coincident).toMatchObject({
      value: { notes: 'canonical second revision' },
      baseline: { notes: 'canonical second revision' },
      revision: 2,
      acknowledgedRevision: 1,
      unacknowledgedRevision: 2,
      inFlight: null,
    });
  });

  it('clears a coincident pending revision only after a later edit returns to its known baseline', () => {
    const d0 = createDraft(scope, target, { notes: 'saved' });
    const d1 = revise(d0, { notes: 'first revision' });
    const a1 = beginAttempt(d1, 1);
    const d2 = revise(a1.draft, { notes: 'canonical second revision' });
    const coincident = acknowledge(d2, a1.attempt, {
      notes: 'canonical second revision',
    });

    const editedAway = revise(coincident, { notes: 'deliberate third revision' });
    const returned = revise(editedAway, { notes: 'canonical second revision' });

    expect(returned).toMatchObject({
      value: { notes: 'canonical second revision' },
      baseline: { notes: 'canonical second revision' },
      revision: 4,
      acknowledgedRevision: 1,
      inFlight: null,
    });
    expect(isContentDirty(returned)).toBe(false);
    expect(statusOf(returned)).toBe('saved');
  });

  it.each([
    ['wrong scope', { scope: entityScope('camp-b', 'npc', 'mira') }],
    ['wrong target', { target: 'entity:npc:torvin' }],
    ['wrong attempt id', { id: 999 }],
    ['wrong revision', { revision: 999 }],
  ])('ignores an acknowledgment for the %s', (_label, mismatch) => {
    const edited = revise(createDraft(scope, target, { notes: 'saved' }), { notes: 'new' });
    const { draft: saving, attempt } = beginAttempt(edited, 1);

    const result = acknowledge(saving, { ...attempt, ...mismatch }, { notes: 'wrong' });

    expect(result).toEqual(saving);
    expect(statusOf(result)).toBe('saving');
  });

  it.each([
    [
      'a cyclic canonical value',
      () => {
        const cyclic: Record<string, unknown> = { notes: 'invalid' };
        cyclic.self = cyclic;
        return cyclic;
      },
    ],
    ['an unsupported Date canonical value', () => new Date('2026-09-09T00:00:00Z')],
    ['an unsupported Set canonical value', () => new Set(['invalid'])],
  ] satisfies Array<[string, () => unknown]>)(
    'ignores %s for a stale attempt before normalization',
    (_label, canonicalValue) => {
      const edited = revise(createDraft(scope, target, { notes: 'saved' }), {
        notes: 'newer retained edit',
      });
      const { draft: saving, attempt } = beginAttempt(edited, 1);
      const staleAttempt = Object.freeze({ ...attempt, id: attempt.id + 1 });

      expect(() => acknowledge(saving, staleAttempt, canonicalValue() as never)).not.toThrow();
      expect(acknowledge(saving, staleAttempt, canonicalValue() as never)).toBe(saving);
      expect(saving).toMatchObject({
        value: { notes: 'newer retained edit' },
        revision: 1,
        inFlight: attempt,
        error: null,
      });
      expect(statusOf(saving)).toBe('saving');
    },
  );

  it('rejects an invalid canonical value for the applicable save without mutating its draft', () => {
    const edited = revise(createDraft(scope, target, { notes: 'saved' }), {
      notes: 'retained edit',
    });
    const { draft: saving, attempt } = beginAttempt(edited, 1);
    const cyclicCanonical: Record<string, unknown> = { notes: 'invalid' };
    cyclicCanonical.self = cyclicCanonical;

    expect(() => acknowledge(saving, attempt, cyclicCanonical as never)).toThrow(/cycles/i);
    expect(saving).toMatchObject({
      baseline: { notes: 'saved' },
      value: { notes: 'retained edit' },
      revision: 1,
      acknowledgedRevision: 0,
      inFlight: attempt,
      error: null,
    });
    expect(statusOf(saving)).toBe('saving');
  });

  it('retains the latest value and exposes a recoverable failure', () => {
    const edited = revise(createDraft(scope, target, { notes: 'saved' }), { notes: 'first' });
    const { draft: saving, attempt } = beginAttempt(edited, 1);
    const newer = revise(saving, { notes: 'latest' });

    const failed = rejectAttempt(newer, attempt, 'database locked');

    expect(failed).toMatchObject({
      scope,
      target,
      baseline: { notes: 'saved' },
      value: { notes: 'latest' },
      revision: 2,
      acknowledgedRevision: 0,
      inFlight: null,
      error: 'database locked',
    });
    expect(statusOf(failed)).toBe('failed');
  });

  it('retries a failed draft by capturing its latest revision and clearing the old error', () => {
    const edited = revise(createDraft(scope, target, { notes: 'saved' }), { notes: 'first' });
    const first = beginAttempt(edited, 1);
    const failed = rejectAttempt(
      revise(first.draft, { notes: 'latest' }),
      first.attempt,
      'database locked',
    );

    const retry = beginAttempt(failed, 2);

    expect(retry.attempt).toMatchObject({
      id: 2,
      scope,
      target,
      revision: 2,
      value: { notes: 'latest' },
    });
    expect(retry.draft.error).toBeNull();
    expect(statusOf(retry.draft)).toBe('saving');
  });

  it('discards only the intended value and invalidates its in-flight attempt', () => {
    const edited = revise(createDraft(scope, target, { notes: 'saved' }), { notes: 'changed' });
    const { draft: saving, attempt } = beginAttempt(edited, 1);

    const discarded = discard(saving);
    const lateCompletion = acknowledge(discarded, attempt, { notes: 'changed' });

    expect(discarded).toMatchObject({
      scope,
      target,
      baseline: { notes: 'saved' },
      value: { notes: 'saved' },
      inFlight: null,
      error: null,
    });
    expect(statusOf(discarded)).toBe('saved');
    expect(lateCompletion).toEqual(discarded);
  });
});
