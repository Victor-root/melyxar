/*
 * The arithmetic of cutting a song short: how loud it is along its length,
 * where the silence at each end stops, and a cut kept within the song.
 */

import type { SongSpectrum } from "./api";

/** The shortest a song may be cut to, as the server holds it too. */
export const SHORTEST_CUT_SECONDS = 1;

/** Below this share of the loudest band, a moment counts as silence. */
const SILENT = 0.06;

/** Kept on each side of the first and last sound, so a soft attack or a
 *  long reverb is not clipped by a cut made too close. */
const BREATH_SECONDS = 0.15;

/** How loud the song is at each of its moments, from nought to one: the
 *  loudest of its bands, moment by moment. */
export function loudnessOf(spectrum: SongSpectrum): number[] {
  const moments = Math.floor(spectrum.levels.length / spectrum.bands);
  const loudness = new Array<number>(moments);
  for (let moment = 0; moment < moments; moment += 1) {
    let loudest = 0;
    for (let band = 0; band < spectrum.bands; band += 1) {
      loudest = Math.max(loudest, spectrum.levels[moment * spectrum.bands + band]);
    }
    loudness[moment] = loudest / 255;
  }
  return loudness;
}

/** Where the sound of a song starts and stops, in seconds, the silence at
 *  either end left out with a breath kept on each side. Nothing when it is
 *  silent throughout. */
export function soundBetween(
  loudness: number[],
  momentsASecond: number,
  length: number,
): { from: number; to: number } | null {
  const first = loudness.findIndex((level) => level > SILENT);
  if (first < 0 || momentsASecond <= 0) {
    return null;
  }
  let last = loudness.length - 1;
  while (last > first && loudness[last] <= SILENT) {
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
export function keptWithin(from: number, to: number, length: number): { from: number; to: number } {
  const end = Math.min(Math.max(to, SHORTEST_CUT_SECONDS), length);
  const start = Math.min(Math.max(from, 0), Math.max(end - SHORTEST_CUT_SECONDS, 0));
  return { from: start, to: end };
}

/** A moment of a song as minutes, seconds and tenths: 1:05.3. */
export function clockOf(seconds: number): string {
  const tenths = Math.round(Math.max(seconds, 0) * 10);
  const minutes = Math.floor(tenths / 600);
  const rest = (tenths % 600) / 10;
  return `${minutes}:${rest < 10 ? "0" : ""}${rest.toFixed(1)}`;
}
