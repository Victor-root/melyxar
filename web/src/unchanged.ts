/*
 * What a list read again keeps of the list it replaces.
 *
 * A grid read again after a mark gets every card as a new object, equal to
 * the old one in all but a handful, and each new object is a card drawn
 * again. Handing back the old object wherever the two say the same thing
 * leaves those cards exactly as they are.
 */

/** The new list, with each item that says what its old one said replaced by
 *  that old one. Items are told apart by their id. */
export function keepTheUnchanged<T extends { id: string }>(before: readonly T[], after: readonly T[]): T[] {
  const held = new Map(before.map((item) => [item.id, item]));
  return after.map((item) => {
    const old = held.get(item.id);
    return old !== undefined && JSON.stringify(old) === JSON.stringify(item) ? old : item;
  });
}
