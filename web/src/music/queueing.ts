/*
 * Where the songs of a whole library are taken from, for a queue.
 *
 * A queue holds a few thousand songs at most, so in a larger library the ones
 * a shuffle draws from are a stretch of it starting somewhere at random: the
 * same first songs every time would not be much of a shuffle.
 */

/** How many songs a queue holds at most, the same as the server's. */
export const LONGEST_QUEUE = 2000;

/** Where a shuffled queue starts in a library of this many songs, given a
 *  number from nought up to but not including one. */
export function shuffleOffset(total: number, random: number): number {
  const spare = total - LONGEST_QUEUE;
  return spare > 0 ? Math.floor(random * (spare + 1)) : 0;
}
