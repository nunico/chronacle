import type { RuleEntry } from '../commands';
import type { DraftCoordinator } from './draft-coordinator.svelte';

const entriesByCoordinator = new WeakMap<DraftCoordinator, Map<string, RuleEntry>>();

function entriesFor(coordinator: DraftCoordinator): Map<string, RuleEntry> {
  let entries = entriesByCoordinator.get(coordinator);
  if (!entries) {
    entries = new Map();
    entriesByCoordinator.set(coordinator, entries);
  }
  return entries;
}

/** Retain non-editable row presentation while an app-lifetime rule-note draft is at risk. */
export function rememberRuleEntry(
  coordinator: DraftCoordinator,
  scope: string,
  entry: RuleEntry,
): void {
  entriesFor(coordinator).set(scope, {
    ...entry,
    page_refs: entry.page_refs.map((reference) => ({ ...reference })),
  });
}

/** Recover the last presentation for a rule that disappeared from a later list response. */
export function rememberedRuleEntry(
  coordinator: DraftCoordinator,
  scope: string,
): RuleEntry | undefined {
  return entriesByCoordinator.get(coordinator)?.get(scope);
}
