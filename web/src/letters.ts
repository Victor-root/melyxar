/*
 * Which letter a grid read by title is showing at the top of the screen.
 */

/** One card as it stands on the screen, and the letter it is filed under. */
export interface Placed {
  top: number;
  height: number;
  letter: string | undefined;
}

/**
 * The letter most of the top row is filed under, the later one when two share
 * it.
 *
 * The top row is the first one standing mostly below `line`, the lower edge
 * of the bar at the top: one scrolled up by less than a quarter of its height
 * still counts. The cards are read through `at` rather than handed over, and
 * found by halving: they stand in the order of their rows, a grid may hold
 * thousands, and reading where each one stands costs a measurement.
 */
export function letterOfTheTopRow(
  count: number,
  at: (index: number) => Placed,
  line: number,
): string | null {
  let low = 0;
  let high = count;
  while (low < high) {
    const middle = (low + high) >> 1;
    const card = at(middle);
    if (card.top >= line - card.height / 4) {
      high = middle;
    } else {
      low = middle + 1;
    }
  }
  if (low >= count) {
    return null;
  }
  const rowTop = at(low).top;
  const tally = new Map<string, number>();
  let best: string | null = null;
  for (let index = low; index < count; index += 1) {
    const card = at(index);
    if (Math.abs(card.top - rowTop) > 1) {
      break;
    }
    if (card.letter) {
      const counted = (tally.get(card.letter) ?? 0) + 1;
      tally.set(card.letter, counted);
      if (best === null || counted >= (tally.get(best) ?? 0)) {
        best = card.letter;
      }
    }
  }
  return best;
}
