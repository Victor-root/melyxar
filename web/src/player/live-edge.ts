/*
 * Keeping a camera at now, as the application does (see the Android
 * `LiveEdge`): what has come from the camera and is not shown yet is how far
 * behind the picture is. A little behind, it is played faster until caught
 * up; far behind, after the tab was left in the background for instance, it
 * jumps to now.
 */

/** How far behind the camera the picture is let be, in seconds. */
export const BEHIND_AT_MOST = 0.3;

/** How far behind the picture jumps to now rather than catch up. */
export const JUMPED_FROM = 3;

const FASTEST = 1.5;
const FASTER_PER_SECOND_BEHIND = 0.5;
const STEP = 0.05;

/** How fast the picture plays when it is `behind` seconds behind. */
export function liveRate(behind: number): number {
  const over = behind - BEHIND_AT_MOST;
  if (over <= 0) {
    return 1;
  }
  const wanted = Math.min(FASTEST, 1 + over * FASTER_PER_SECOND_BEHIND);
  /* In steps, so the rate is not changed at every look. */
  return Math.round(wanted / STEP) * STEP;
}
