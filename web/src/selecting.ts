/*
 * What a press does to a set of chosen cards.
 *
 * A press alone turns one card on or off. A press with the shift key held
 * chooses every card from the one pressed last to this one, in the order they
 * are shown, which is how fifty episodes are chosen in two presses rather than
 * fifty. It only ever adds: a range that turned some cards off would undo a
 * choice nobody could see being undone.
 */

export function pressed(
  order: readonly string[],
  chosen: ReadonlySet<string>,
  id: string,
  /** The card pressed before this one, where a range starts. */
  from: string | null,
  range: boolean,
): Set<string> {
  const next = new Set(chosen);
  const start = from === null ? -1 : order.indexOf(from);
  const end = order.indexOf(id);
  if (range && start >= 0 && end >= 0) {
    const [low, high] = start < end ? [start, end] : [end, start];
    for (const between of order.slice(low, high + 1)) {
      next.add(between);
    }
    return next;
  }
  if (next.has(id)) {
    next.delete(id);
  } else {
    next.add(id);
  }
  return next;
}
