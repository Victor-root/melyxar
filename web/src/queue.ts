/*
 * What is played one after the other once somebody asked to play everything
 * from a card onwards: the cards of that row or grid, in its order, from the
 * one pressed.
 *
 * Kept outside any page, since each work is played on its own page and the
 * sitting goes from one page to the next. It lasts as long as the sitting:
 * leaving the player forgets it.
 */

/** The works to play in turn, and which of them is being played. */
export interface Queue {
  ids: string[];
  at: number;
}

let held: Queue | null = null;

/** Plays these works in turn, starting with the first. */
export function playInTurn(ids: string[]): void {
  held = ids.length > 1 ? { ids, at: 0 } : null;
}

/** Forgets what was to be played in turn. */
export function forgetTheQueue(): void {
  held = null;
}

/** The queue as it stands, for the page that decides what comes next. */
export function theQueue(): Queue | null {
  return held;
}

/** Moves the queue on to the work about to be played. */
export function movedTo(queue: Queue, id: string): void {
  const at = queue.ids.indexOf(id, queue.at);
  if (at >= 0) {
    held = { ids: queue.ids, at };
  }
}

/**
 * What to play once this work ends, or nothing.
 *
 * A work the queue named is followed by the next one it names. An episode
 * the queue did not name belongs to a series it did, which carries on with
 * its own next episode; once the series has none left, the queue carries on
 * after it. Without a queue, an episode's next episode is all there is.
 */
export function whatComesNext(
  queue: Queue | null,
  workId: string,
  nextEpisode: string | null,
): string | null {
  const following = queue ? (queue.ids[queue.at + 1] ?? null) : null;
  if (queue && queue.ids[queue.at] === workId) {
    return following;
  }
  return nextEpisode ?? following;
}
