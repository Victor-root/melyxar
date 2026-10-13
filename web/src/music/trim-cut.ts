/*
 * The arithmetic of cutting a song short: where the silence at each end
 * stops, a cut kept within the song, and the stretch of it the bar shows.
 */

import type { SongWave } from "./api";

/** A stretch of the song, in seconds. */
export interface Stretch {
  from: number;
  to: number;
}

/** The shortest a song may be cut to, as the server holds it too. */
export const SHORTEST_CUT_SECONDS = 1;

/** The shortest stretch the bar zooms in to. */
export const CLOSEST_SECONDS = 1.5;

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

/** The stretch shown once zoomed by `factor` (over one is closer), the
 *  moment at `around` staying where it was on the bar. */
export function zoomed(shown: Stretch, length: number, factor: number, around: number): Stretch {
  const span = shown.to - shown.from;
  const next = Math.min(Math.max(span / factor, Math.min(CLOSEST_SECONDS, length)), length);
  const share = span > 0 ? (around - shown.from) / span : 0.5;
  return placed(around - share * next, next, length);
}

/** The stretch shown moved along by `by` seconds, never past either end. */
export function panned(shown: Stretch, length: number, by: number): Stretch {
  return placed(shown.from + by, shown.to - shown.from, length);
}

/** The stretch shown, moved just enough for `at` to be on it with a little
 *  room to spare. */
export function showing(shown: Stretch, length: number, at: number): Stretch {
  const span = shown.to - shown.from;
  const room = span * 0.1;
  if (at < shown.from + room) {
    return placed(at - room, span, length);
  }
  if (at > shown.to - room) {
    return placed(at + room - span, span, length);
  }
  return shown;
}

function placed(from: number, span: number, length: number): Stretch {
  const start = Math.min(Math.max(from, 0), Math.max(length - span, 0));
  return { from: start, to: start + span };
}

/** How tall each of `count` bars is drawn, from nought to one, for the
 *  stretch shown: the loudest of the moments each covers, or the one it
 *  falls on when zoomed in closer than a moment a bar. */
export function barsOf(wave: SongWave, shown: Stretch, count: number): number[] {
  const { levels, momentsASecond } = wave;
  const bars = new Array<number>(count).fill(0);
  const span = shown.to - shown.from;
  for (let bar = 0; bar < count; bar += 1) {
    const first = Math.floor((shown.from + (bar / count) * span) * momentsASecond);
    const last = Math.max(first + 1, Math.floor((shown.from + ((bar + 1) / count) * span) * momentsASecond));
    let loudest = 0;
    for (let moment = Math.max(first, 0); moment < last && moment < levels.length; moment += 1) {
      loudest = Math.max(loudest, levels[moment]);
    }
    bars[bar] = loudest / 255;
  }
  return bars;
}

/** A moment of a song as minutes, seconds and tenths: 1:05.3. */
export function clockOf(seconds: number): string {
  const tenths = Math.round(Math.max(seconds, 0) * 10);
  const minutes = Math.floor(tenths / 600);
  const rest = (tenths % 600) / 10;
  return `${minutes}:${rest < 10 ? "0" : ""}${rest.toFixed(1)}`;
}
