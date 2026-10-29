/*
 * The arithmetic of a bar that draws a song's wave: the stretch of the song
 * it shows, zoomed and moved along, and how tall each of its bars is drawn.
 */

import type { SongWave } from "./api";

/** A stretch of the song, in seconds. */
export interface Stretch {
  from: number;
  to: number;
}

/** The shortest stretch the bar zooms in to. */
export const CLOSEST_SECONDS = 1.5;

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

/** The stretch shown, turned like a page when `at` runs out of it: `at`
 *  then stands a little way in from the start. Left alone otherwise, so
 *  what follows a song playing is not drawn again at every moment. */
export function paged(shown: Stretch, length: number, at: number): Stretch {
  const span = shown.to - shown.from;
  const room = span * 0.1;
  return at >= shown.from && at <= shown.to - room ? shown : placed(at - room, span, length);
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

/** How far the sound swings at a level of the wave, from nought to one, the
 *  loudest level of the song swinging all the way: the levels are decibels
 *  over sixty, which show a fade and a silence plainly but draw a song
 *  mastered loud as a block, where the swing shows each attack. */
export function swingOf(level: number, loudest = 1): number {
  return level <= 0 || loudest <= 0 ? 0 : Math.min(1, 10 ** ((level - loudest) * 3));
}

/** The steps a ruler is written in, in seconds, the finest first. */
const RULER_STEPS = [0.1, 0.2, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300];

/** Where a ruler over the stretch shown is marked, `width` pixels wide: a
 *  moment written every `step` seconds, the finest step that keeps them
 *  `room` pixels apart, and a small tick at each fifth of a step between. */
export function rulerOf(shown: Stretch, width: number, room = 80): { step: number; written: number[]; ticks: number[] } {
  const span = shown.to - shown.from;
  const wanted = width > 0 ? (room * span) / width : span;
  const step = RULER_STEPS.find((one) => one >= wanted) ?? RULER_STEPS[RULER_STEPS.length - 1];
  const every = (gap: number) => {
    const found: number[] = [];
    for (let index = Math.ceil(shown.from / gap - 1e-9); index * gap <= shown.to + 1e-9; index += 1) {
      found.push(Math.round(index * gap * 1e6) / 1e6 + 0);
    }
    return found;
  };
  const written = every(step);
  const ticks = every(step / 5).filter((at) => Math.abs(at / step - Math.round(at / step)) > 1e-6);
  return { step, written, ticks };
}

/** A moment of a song as minutes, seconds and tenths: 1:05.3. */
export function clockOf(seconds: number): string {
  const tenths = Math.round(Math.max(seconds, 0) * 10);
  const minutes = Math.floor(tenths / 600);
  const rest = (tenths % 600) / 10;
  return `${minutes}:${rest < 10 ? "0" : ""}${rest.toFixed(1)}`;
}
