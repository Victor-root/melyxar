/*
 * A film whose sound stops for a while in the file itself: the browser has
 * nothing to play there and stands still at it, picture held ahead and
 * nothing moving. Seen standing still that way for long enough, the player
 * asks the server where the sound comes back and goes on from there.
 */

/** How long the film stands still, in milliseconds, before it is helped. */
export const STILL_FOR = 2000;

/** How much film must be held past the moment it stands at, in seconds:
 *  without it the film is simply waiting for the network. */
export const HELD_AHEAD = 1;

/** Where the film was last seen, and since when it has stood there. */
export type Watch = { at: number; since: number } | null;

/** Takes in where the film stands `now`, and says whether it has stood still
 *  long enough, with film held ahead, to be stuck at a hole. */
export function watchStill(
  watch: Watch,
  at: number,
  now: number,
  heldAhead: number,
): { watch: Watch; stuck: boolean } {
  if (watch === null || watch.at !== at) {
    return { watch: { at, since: now }, stuck: false };
  }
  return { watch, stuck: now - watch.since >= STILL_FOR && heldAhead > HELD_AHEAD };
}
