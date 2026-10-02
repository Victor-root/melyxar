/*
 * Where the wave behind the player stands, from the levels the server kept
 * for the song and where the song has got to: plain arithmetic, so the part
 * that draws has only to draw.
 */

import type { SongSpectrum } from "../api";

/** How much of the way to its target a level goes in one drawing. */
const GLIDE = 0.22;
/** Below this, a level is taken for settled. */
const SETTLED = 0.004;

/**
 * The levels at this moment of the song, each from nought to one, between the
 * two readings it falls between. A reading stands for the stretch it was made
 * over, so it is read at the middle of that stretch. Before the first reading
 * and after the last, the nearest holds.
 */
export function levelsAt(spectrum: SongSpectrum, seconds: number, into: number[]): void {
  const frames = spectrum.levels.length / spectrum.bands;
  const at = Math.min(frames - 1, Math.max(0, seconds * spectrum.framesASecond - 0.5));
  const before = Math.floor(at);
  const after = Math.min(frames - 1, before + 1);
  const share = at - before;
  for (let band = 0; band < spectrum.bands; band += 1) {
    const first = spectrum.levels[before * spectrum.bands + band];
    const second = spectrum.levels[after * spectrum.bands + band];
    into[band] = (first + (second - first) * share) / 255;
  }
}

/** One step of every level towards its target, and whether all have settled
 *  at the target. */
export function approach(levels: number[], targets: number[]): boolean {
  let settled = true;
  for (let band = 0; band < levels.length; band += 1) {
    levels[band] += (targets[band] - levels[band]) * GLIDE;
    if (Math.abs(targets[band] - levels[band]) > SETTLED) {
      settled = false;
    }
  }
  return settled;
}

/**
 * Where the song is, to the fraction of a second, from the whole second the
 * player last said and how long ago it said it. The player says it as the
 * second turns, so the song stands in that second's first quarter; the time
 * that has gone since is added, but never past the next second, which the
 * player would have said.
 */
export function momentOf(position: number, sinceItWasSaidMs: number): number {
  const FIRST_QUARTER = 0.125;
  return position + FIRST_QUARTER + Math.min(1, Math.max(0, sinceItWasSaidMs / 1000));
}
