/**
 * Decide whether leaving an editor field should trigger its automatic save.
 *
 * Moving outside the editing scope is navigation, so the draft is retained
 * without starting a write. A blur without a destination (for example a
 * programmatic or window blur) keeps the editor's ordinary auto-save behavior.
 */
export function shouldAutoSaveAfterBlur(
  editingScope: Node | null | undefined,
  nextTarget: EventTarget | null,
  retainWithoutSaveTargets: ReadonlyArray<Node | null | undefined> = [],
): boolean {
  if (nextTarget === null) return true;
  if (!(nextTarget instanceof Node)) return false;
  if (!editingScope?.contains(nextTarget)) return false;

  return !retainWithoutSaveTargets.some((target) => target?.contains(nextTarget));
}
