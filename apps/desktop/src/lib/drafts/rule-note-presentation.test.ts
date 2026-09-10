import { describe, expect, it } from 'vitest';
import { DraftCoordinator } from './draft-coordinator.svelte';
import {
  forgetRuleRecovery,
  forgetRuleRecoveryPrefix,
  rememberedRuleRecovery,
  rememberRuleRecovery,
} from './rule-note-presentation';

describe('rule-note recovery presentation', () => {
  it('retains only the identity and label needed to reopen an at-risk note', () => {
    const coordinator = new DraftCoordinator();
    const scope = 'rule:camp-a:book-a:initiative';

    rememberRuleRecovery(coordinator, scope, {
      ruleId: 'initiative',
      title: 'Initiative',
      collectionId: 'book-a',
    });

    const recovery = rememberedRuleRecovery(coordinator, scope);
    expect(recovery).toEqual({
      ruleId: 'initiative',
      title: 'Initiative',
      collectionId: 'book-a',
    });
    expect(Object.keys(recovery ?? {}).sort()).toEqual(['collectionId', 'ruleId', 'title']);
    expect(Object.isFrozen(recovery)).toBe(true);
  });

  it('forgets exact and prefixed metadata without touching neighboring scopes', () => {
    const coordinator = new DraftCoordinator();
    const first = 'rule:camp-a:book-a:initiative';
    const second = 'rule:camp-a:book-a:surprise';
    const otherCollection = 'rule:camp-a:book-b:initiative';
    const otherCampaign = 'rule:camp-b:book-a:initiative';
    for (const [scope, ruleId, collectionId] of [
      [first, 'initiative', 'book-a'],
      [second, 'surprise', 'book-a'],
      [otherCollection, 'initiative', 'book-b'],
      [otherCampaign, 'initiative', 'book-a'],
    ]) {
      rememberRuleRecovery(coordinator, scope, { ruleId, title: ruleId, collectionId });
    }

    expect(forgetRuleRecovery(coordinator, first)).toBe(true);
    expect(rememberedRuleRecovery(coordinator, first)).toBeUndefined();
    expect(forgetRuleRecoveryPrefix(coordinator, 'rule:camp-a:book-a:')).toBe(1);
    expect(rememberedRuleRecovery(coordinator, second)).toBeUndefined();
    expect(rememberedRuleRecovery(coordinator, otherCollection)).toBeDefined();
    expect(rememberedRuleRecovery(coordinator, otherCampaign)).toBeDefined();
  });
});
