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

/** How many bars the button of the song playing has. */
const BARS = 4;

/** The heights of the bars of the button of the song playing, from the
 *  levels of the wave: the bands shared out between them in order, the lows
 *  first, each the loudest of its share, so whatever stands out of the song
 *  lifts its bar. */
export function barsOf(levels: readonly number[]): number[] {
  const bars = new Array<number>(BARS).fill(0);
  for (let bar = 0; bar < BARS && levels.length > 0; bar += 1) {
    const first = Math.floor((bar * levels.length) / BARS);
    const last = Math.max(first + 1, Math.floor(((bar + 1) * levels.length) / BARS));
    for (let band = first; band < last; band += 1) {
      bars[bar] = Math.max(bars[bar], levels[band]);
    }
  }
  return bars;
}

/** How long a bar takes to forget the loudest and the quietest it has seen,
 *  which is what it is spread between. */
const GAIN_MEMORY_MS = 4000;
/** The least a bar is spread over, so a song holding still does not make
 *  noise of nothing. */
const LEAST_SPREAD = 0.12;
/** How much of the way up a bar goes in one step, and how fast it falls, in
 *  heights a second: up at once, down slowly, as a level meter does. */
const ATTACK = 0.7;
const FALL_A_SECOND = 1.6;
/** The lowest a bar stands, never quite flat. */
const FLOOR = 0.15;

/**
 * The bars of the button of the song playing as they move: each spread
 * between the loudest and the quietest of its own recent seconds, so a song
 * that keeps to a narrow range still moves them all the way, then jumping up
 * at once and falling back slowly.
 */
export class BarMeter {
  private readonly low = new Array<number>(BARS).fill(1);
  private readonly high = new Array<number>(BARS).fill(0);
  private readonly shown = new Array<number>(BARS).fill(FLOOR);

  /** The heights to show now, from nought to one, given the bars read from
   *  the wave and how long it is since the last step. */
  step(read: readonly number[], sinceMs: number): readonly number[] {
    const forget = Math.min(1, sinceMs / GAIN_MEMORY_MS);
    for (let bar = 0; bar < BARS; bar += 1) {
      const level = read[bar] ?? 0;
      this.high[bar] = level > this.high[bar] ? level : this.high[bar] + (level - this.high[bar]) * forget;
      this.low[bar] = level < this.low[bar] ? level : this.low[bar] + (level - this.low[bar]) * forget;
      const spread = Math.max(this.high[bar] - this.low[bar], LEAST_SPREAD);
      const wanted = Math.min(1, Math.max(FLOOR, (level - this.low[bar]) / spread));
      const shown = this.shown[bar];
      this.shown[bar] =
        wanted > shown ? shown + (wanted - shown) * ATTACK : Math.max(wanted, shown - (FALL_A_SECOND * sinceMs) / 1000);
    }
    return this.shown;
  }
}
