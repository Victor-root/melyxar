/*
 * How the automatic quality picks its rung, and when it moves to another.
 *
 * The connection is measured, never guessed, and the rung taken is the best
 * one whose rate fits in part of what was measured: a connection is never as
 * steady as one measure of it, and a film fed at the edge of what it carries
 * stops at its first dip.
 *
 * Every move to another rung opens the film again where it stands, which a
 * viewer sees for a few seconds. So the rung comes down as soon as the film is
 * in danger and goes back up only once the connection has proved itself for a
 * while: going back and forth is worse than either rung.
 *
 * Nothing here touches the film or the network. It is handed what was
 * measured and says what to do, which is what the tests next to it check.
 */

import { QUALITIES } from "./quality";
import type { Quality } from "./quality";

/** The rungs the automatic quality moves between, lightest first. */
export const ADAPTIVE_RUNGS: Quality[] = [
  ...QUALITIES.filter((quality) => quality.bitrate !== null)
    .reverse()
    .map((quality) => ({ ...quality, key: `adaptive:${quality.key}` })),
  // Above the ladder, the film at its own size with a rate a fast home
  // connection carries, and past that the film as it is.
  { key: "adaptive:40", height: null, bitrate: 40_000_000 },
  { key: "adaptive:as-it-is", height: null, bitrate: null },
];

/** The rung of the film as it is, the last one. */
const AS_IT_IS_RUNG = ADAPTIVE_RUNGS.length - 1;

/** What the connection has to carry for the film as it is to be offered: no
 *  rate can be read off a film before it is asked for, and this is more
 *  than nearly any film is written at. */
const AS_IT_IS_NEEDS = 80_000_000;

/** The part of the measured speed a rung may take. */
const SHARE = 0.7;

/** The rung taken when the connection could not be measured: a picture that
 *  plays on most connections, which the film then corrects either way. */
export const UNMEASURED_RUNG = ADAPTIVE_RUNGS.findIndex((rung) => rung.bitrate === 4_000_000);

/** What a rung needs the connection to carry, in bits a second. */
function needs(rung: number): number {
  return rung === AS_IT_IS_RUNG ? AS_IT_IS_NEEDS : (ADAPTIVE_RUNGS[rung].bitrate ?? AS_IT_IS_NEEDS);
}

/** The best rung a connection this fast carries, the lightest when none fits. */
export function rungFor(bitsPerSecond: number): number {
  const room = bitsPerSecond * SHARE;
  let fits = 0;
  for (let rung = 0; rung < ADAPTIVE_RUNGS.length; rung += 1) {
    if (needs(rung) <= room) {
      fits = rung;
    }
  }
  return fits;
}

/**
 * What the connection carries, followed as segments arrive.
 *
 * Two averages, one quick to move and one slow, and the lower of the two is
 * believed: a sudden drop is seen at once, and a sudden rise has to last
 * before it counts. Each measure weighs as long as it took: a segment that
 * crawled in over eight seconds says more about the connection than one that
 * arrived in a tenth, and a connection that falls from two hundred megabits
 * to three is believed after one such segment rather than after ten.
 */
export interface Speed {
  quick: number;
  slow: number;
}

/** How many seconds of measuring halve the weight of what came before. */
const QUICK_HALF_S = 2;
const SLOW_HALF_S = 5;

/** The speed once one more measure of it is in, taken over that many
 *  seconds. */
export function withMeasure(speed: Speed | null, bitsPerSecond: number, seconds: number): Speed {
  if (speed === null) {
    return { quick: bitsPerSecond, slow: bitsPerSecond };
  }
  const moved = (half: number) => 1 - 0.5 ** (seconds / half);
  return {
    quick: speed.quick + moved(QUICK_HALF_S) * (bitsPerSecond - speed.quick),
    slow: speed.slow + moved(SLOW_HALF_S) * (bitsPerSecond - speed.slow),
  };
}

/** What the connection is believed to carry. */
export function believed(speed: Speed): number {
  return Math.min(speed.quick, speed.slow);
}

/** What the film being played is written at, as far as it is known. */
export interface Film {
  bitrate: number | null;
  height: number | null;
}

/**
 * Whether a rung lets the film through as it is: no taller and no heavier
 * than the rung allows, which the server then hands over without rebuilding
 * it. Two rungs that both let it through give the same film, and moving
 * between them would open it again for nothing.
 */
export function letsThrough(film: Film | null, rung: number): boolean {
  const { bitrate, height } = ADAPTIVE_RUNGS[rung];
  if (bitrate === null) {
    return true;
  }
  if (film === null || film.bitrate === null) {
    return false;
  }
  return film.bitrate <= bitrate && (height === null || (film.height !== null && film.height <= height));
}

/** Where the automatic quality stands, and what it has been through since it
 *  last moved. */
export interface Course {
  rung: number;
  /** When it last moved, and when it last came down, in milliseconds. */
  movedAt: number;
  loweredAt: number | null;
  /** How many segments were measured on this rung. */
  measured: number;
  /** When the film stopped to wait on this rung. */
  stalls: number[];
}

export function startingOn(rung: number, now: number): Course {
  return { rung, movedAt: now, loweredAt: null, measured: 0, stalls: [] };
}

/** Why the rung moved. */
export type Because = "stalls" | "slower_link" | "faster_link";

export interface Move {
  rung: number;
  because: Because;
}

/** Stalls further apart than this say nothing about each other. */
const STALLS_WITHIN_MS = 60_000;
/** How many stalls in that time bring the rung down. */
const STALLS_TOO_MANY = 2;
/** Measures on a rung before its speed is believed. */
const MEASURED_BEFORE_MOVING = 3;
/** Below this much film held ahead, a slow connection is a danger now. */
const HELD_LOW_S = 15;
/** How long a rung is kept before going up, and how long after coming down. */
const KEPT_BEFORE_RISING_MS = 30_000;
const KEPT_AFTER_LOWERING_MS = 120_000;
/** How much film held ahead going up needs, since the first seconds of a new
 *  rung are spent opening it. */
const HELD_TO_RISE_S = 10;

/** The highest rung under this one that changes the film, the lightest
 *  when every one lets it through. */
function lighterThan(film: Film | null, rung: number): number {
  let lighter = rung - 1;
  while (lighter > 0 && letsThrough(film, rung) && letsThrough(film, lighter)) {
    lighter -= 1;
  }
  return lighter;
}

/**
 * Where the rung goes now, or nothing when it stays.
 *
 * `speed` is what the connection is believed to carry, null while nothing was
 * measured on this film. `heldAhead` is how many seconds of film the browser
 * holds past where it plays. `film` is what the film is written at, which
 * says whether a move would change anything at all.
 */
export function nextMove(
  course: Course,
  speed: number | null,
  heldAhead: number,
  now: number,
  film: Film | null,
): Move | null {
  const recentStalls = course.stalls.filter((at) => now - at <= STALLS_WITHIN_MS).length;
  const fits = speed === null ? null : rungFor(speed);
  if (recentStalls >= STALLS_TOO_MANY && course.rung > 0) {
    return { rung: Math.min(lighterThan(film, course.rung), fits ?? course.rung), because: "stalls" };
  }
  if (fits === null || course.measured < MEASURED_BEFORE_MOVING) {
    return null;
  }
  // A film the connection carries as it is needs nothing lighter, and a
  // film already handed over as it is gains nothing from a higher rung.
  if (fits < course.rung && heldAhead < HELD_LOW_S && !letsThrough(film, fits)) {
    return { rung: fits, because: "slower_link" };
  }
  const settled =
    now - course.movedAt >= KEPT_BEFORE_RISING_MS &&
    (course.loweredAt === null || now - course.loweredAt >= KEPT_AFTER_LOWERING_MS);
  if (fits > course.rung && settled && heldAhead >= HELD_TO_RISE_S && !letsThrough(film, course.rung)) {
    return { rung: fits, because: "faster_link" };
  }
  return null;
}

/** The course once the rung has moved. */
export function afterMoving(course: Course, move: Move, now: number): Course {
  return {
    rung: move.rung,
    movedAt: now,
    loweredAt: move.rung < course.rung ? now : course.loweredAt,
    measured: 0,
    stalls: [],
  };
}
