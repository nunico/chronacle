/**
 * Decide whether leaving an editor field should trigger its automatic save.
 *
 * Moving outside the editing scope is navigation, so the draft is retained
 * without starting a write. A blur caused by window deactivation also retains
 * the draft, because close coordination has not necessarily started yet.
 */
export function shouldAutoSaveAfterBlur(
  editingScope: Node | null | undefined,
  nextTarget: EventTarget | null,
  retainWithoutSaveTargets: ReadonlyArray<Node | null | undefined> = [],
  documentHasFocus = true,
): boolean {
  if (nextTarget === null) return documentHasFocus;
  if (!(nextTarget instanceof Node)) return false;
  if (!editingScope?.contains(nextTarget)) return false;

  return !retainWithoutSaveTargets.some((target) => target?.contains(nextTarget));
}
