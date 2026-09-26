/*
 * The arithmetic of a list reordered by hand: where a line being dragged
 * would land, how far each of the others steps aside to make room for it,
 * and the list once it is dropped.
 *
 * Kept apart from the drawing because a slip here looks like a list that
 * refuses the drop, or lands a line one place off, and the eye does not
 * tell which.
 */

/** Where one line of the list sat when the drag began, from the top of the
 *  list. */
export interface Line {
  top: number;
  height: number;
}

/**
 * The place a line would take if it were dropped now, among all of them.
 *
 * A neighbour is passed once the line held covers half of it: its foot past
 * the middle of one below, its head past the middle of one above. That is
 * what a hand means by "past it", and it is where the neighbour has to step
 * aside for the line to fit.
 */
export function landingPlace(lines: readonly Line[], from: number, moved: number): number {
  const head = lines[from].top + moved;
  const foot = head + lines[from].height;
  let place = 0;
  lines.forEach((line, index) => {
    const middle = line.top + line.height / 2;
    if ((index < from && head >= middle) || (index > from && foot > middle)) {
      place += 1;
    }
  });
  return place;
}

/**
 * How far one line is drawn from where it sits, while another is dragged
 * from `from` towards `to` by `moved`.
 *
 * The line held follows the hand. The ones between where it was and where it
 * would land step aside by its height, up or down; the rest stay put.
 */
export function stepAside(
  index: number,
  from: number,
  to: number,
  moved: number,
  height: number,
): number {
  if (index === from) {
    return moved;
  }
  if (from < index && index <= to) {
    return -height;
  }
  if (to <= index && index < from) {
    return height;
  }
  return 0;
}

/** How far the line dropped still is from its place once the list is
 *  reordered, which is what it glides over to settle. */
export function leftToSettle(lines: readonly Line[], from: number, to: number, moved: number): number {
  const held = lines[from];
  const settles =
    to > from ? lines[to].top + lines[to].height - held.height : to < from ? lines[to].top : held.top;
  return held.top + moved - settles;
}

/** The list with one of its entries taken from one place and put at another. */
export function movedWithin<T>(list: readonly T[], from: number, to: number): T[] {
  const reordered = [...list];
  const [taken] = reordered.splice(from, 1);
  reordered.splice(to, 0, taken);
  return reordered;
}
