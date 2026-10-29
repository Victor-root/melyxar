/*
 * The arithmetic of cutting a song short: where the silence at each end
 * stops, and a cut kept within the song.
 */

import type { SongWave } from "./api";
import type { Stretch } from "./wave-stretch";

/** The shortest a song may be cut to, as the server holds it too. */
export const SHORTEST_CUT_SECONDS = 1;

/** At or under this, a moment counts as silence: 48 decibels under full
 *  scale, over the hiss of an old recording and under the softest note. */
const SILENT = Math.round(((60 - 48) / 60) * 255);

/** Kept on each side of the first and last sound, so a soft attack or a
 *  long reverb is not clipped by a cut made too close. */
const BREATH_SECONDS = 0.15;

/** Where the sound of a song starts and stops, in seconds, the silence at
 *  either end left out with a breath kept on each side. Nothing when it is
 *  silent throughout. */
export function soundBetween(wave: SongWave, length: number): Stretch | null {
  const { levels, momentsASecond } = wave;
  const first = levels.findIndex((level) => level > SILENT);
  if (first < 0 || momentsASecond <= 0) {
    return null;
  }
  let last = levels.length - 1;
  while (last > first && levels[last] <= SILENT) {
    last -= 1;
  }
  return keptWithin(
    first / momentsASecond - BREATH_SECONDS,
    (last + 1) / momentsASecond + BREATH_SECONDS,
    length,
  );
}

/** A cut brought within the song and kept at least the shortest long, its
 *  start moved rather than its end when the two come too close. */
export function keptWithin(from: number, to: number, length: number): Stretch {
  const end = Math.min(Math.max(to, SHORTEST_CUT_SECONDS), length);
  const start = Math.min(Math.max(from, 0), Math.max(end - SHORTEST_CUT_SECONDS, 0));
  return { from: start, to: end };
}
