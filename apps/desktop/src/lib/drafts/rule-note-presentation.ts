import type { DraftCoordinator } from './draft-coordinator.svelte';

export interface RuleRecoveryPresentation {
  readonly ruleId: string;
  readonly title: string;
  readonly collectionId: string;
}

const entriesByCoordinator = new WeakMap<DraftCoordinator, Map<string, RuleRecoveryPresentation>>();

function entriesFor(coordinator: DraftCoordinator): Map<string, RuleRecoveryPresentation> {
  let entries = entriesByCoordinator.get(coordinator);
  if (!entries) {
    entries = new Map();
    entriesByCoordinator.set(coordinator, entries);
  }
  return entries;
}

/** Retain only the identity and label needed to present an at-risk rule-note draft. */
export function rememberRuleRecovery(
  coordinator: DraftCoordinator,
  scope: string,
  recovery: RuleRecoveryPresentation,
): void {
  entriesFor(coordinator).set(
    scope,
    Object.freeze({
      ruleId: recovery.ruleId,
      title: recovery.title,
      collectionId: recovery.collectionId,
    }),
  );
}

/** Recover the minimal presentation for a rule that disappeared from a later list response. */
export function rememberedRuleRecovery(
  coordinator: DraftCoordinator,
  scope: string,
): RuleRecoveryPresentation | undefined {
  return entriesByCoordinator.get(coordinator)?.get(scope);
}

/** Remove minimal recovery presentation after exact-scope deletion or release. */
export function forgetRuleRecovery(coordinator: DraftCoordinator, scope: string): boolean {
  return entriesByCoordinator.get(coordinator)?.delete(scope) ?? false;
}

/** Remove minimal recovery presentation after collection or campaign teardown. */
export function forgetRuleRecoveryPrefix(coordinator: DraftCoordinator, prefix: string): number {
  if (prefix.length === 0) throw new Error('Rule recovery prefix cannot be empty');
  const entries = entriesByCoordinator.get(coordinator);
  if (!entries) return 0;
  let removed = 0;
  for (const scope of [...entries.keys()]) {
    const matches =
      scope === prefix ||
      (prefix.endsWith(':') ? scope.startsWith(prefix) : scope.startsWith(`${prefix}:`));
    if (!matches) continue;
    entries.delete(scope);
    removed += 1;
  }
  if (entries.size === 0) entriesByCoordinator.delete(coordinator);
  return removed;
}
