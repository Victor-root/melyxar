/*
 * A film whose picture stops a little before the length it announces: the
 * sound runs on past the last picture, the browser holds only what has both,
 * and it stands still a second short of the end. It never says the film
 * ended, so nothing follows it, not the next episode either. Seen standing
 * still that close to its end, with no more than a moment held after, the
 * film is over.
 */

import { HELD_AHEAD, STILL_FOR } from "./sound-hole";
import type { Watch } from "./sound-hole";

/** How close to its announced length a film standing still counts as over,
 *  in seconds: the sound running past the last picture, a fraction of a
 *  second to a second, and a last piece of film the browser would not take,
 *  which is about a second more. */
export const THE_LAST_STRETCH = 3;

/** Whether the film, seen at `at` since the watch began, has stood still at
 *  its very end long enough to be over. */
export function standsAtItsEnd(
  watch: Watch,
  at: number,
  now: number,
  length: number,
  heldAhead: number,
): boolean {
  return (
    watch !== null &&
    watch.at === at &&
    now - watch.since >= STILL_FOR &&
    Number.isFinite(length) &&
    length > 0 &&
    length - at <= THE_LAST_STRETCH &&
    // More held than this is a hole in the sound, which `sound-hole.ts` sees
    // to; less, and the browser stands still on the last of it, seen at a
    // third of a second short as often as at nothing.
    heldAhead <= HELD_AHEAD
  );
}
