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

  it('projects only allowed metadata from an object with forbidden runtime fields', () => {
    const coordinator = new DraftCoordinator();
    const scope = 'rule:camp-a:book-a:initiative';
    const untrustedPresentation = {
      ruleId: 'initiative',
      title: 'Initiative',
      collectionId: 'book-a',
      body: 'Private compiled text',
      notes: 'Private GM note',
      pageRefs: [{ source: 'Secret source', page: 42 }],
    };

    rememberRuleRecovery(coordinator, scope, untrustedPresentation);

    expect(rememberedRuleRecovery(coordinator, scope)).toEqual({
      ruleId: 'initiative',
      title: 'Initiative',
      collectionId: 'book-a',
    });
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

  it('uses a delimiter boundary when forgetting a non-delimited prefix', () => {
    const coordinator = new DraftCoordinator();
    const intended = 'rule:camp-a:book-a:initiative';
    const longerCampaign = 'rule:camp-ab:book-a:initiative';
    const suffixedCampaign = 'rule:camp-a-long:book-a:initiative';
    for (const [scope, campaign] of [
      [intended, 'camp-a'],
      [longerCampaign, 'camp-ab'],
      [suffixedCampaign, 'camp-a-long'],
    ]) {
      rememberRuleRecovery(coordinator, scope, {
        ruleId: 'initiative',
        title: campaign,
        collectionId: 'book-a',
      });
    }

    expect(forgetRuleRecoveryPrefix(coordinator, 'rule:camp-a')).toBe(1);
    expect(rememberedRuleRecovery(coordinator, intended)).toBeUndefined();
    expect(rememberedRuleRecovery(coordinator, longerCampaign)).toBeDefined();
    expect(rememberedRuleRecovery(coordinator, suffixedCampaign)).toBeDefined();
  });
});
