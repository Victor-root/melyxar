/*
 * Which lines of a long list of lines all of one height are made, from where
 * the box it scrolls in stands: the ones in sight and a margin of them on
 * either side, so a queue of two thousand songs is a few dozen lines on the
 * page whatever its length.
 *
 * The edges move by whole steps of lines rather than with every line that
 * scrolls by, so the list is drawn again once every few lines, not on every
 * frame of a scroll.
 */

/** How many lines are made beyond those in sight, on each side. */
export const MARGIN = 10;

/** By how many lines the edges move at a time. */
export const STEP = 10;

/** The lines made, from `from` up to but not including `to`. */
export interface Shown {
  from: number;
  to: number;
}

/**
 * The lines to make for a list of `count` lines of `line` points each that
 * begins `listTop` points down the box, when the box is scrolled to `top` and
 * shows `height` points of it.
 */
export function linesShown(top: number, height: number, listTop: number, line: number, count: number): Shown {
  if (line <= 0 || count === 0) {
    return { from: 0, to: Math.min(count, STEP) };
  }
  const first = Math.floor((top - listTop) / line);
  const last = Math.ceil((top + height - listTop) / line);
  const from = Math.max(0, Math.floor((first - MARGIN) / STEP) * STEP);
  const to = Math.min(count, Math.ceil((last + MARGIN) / STEP) * STEP);
  return from < to ? { from, to } : { from: Math.max(0, Math.min(from, count - STEP)), to: count };
}

/** The lines around one, before anything is measured: enough to fill a tall
 *  screen on either side of it. */
export function linesAround(at: number, count: number): Shown {
  const from = Math.max(0, Math.floor((at - 2 * MARGIN) / STEP) * STEP);
  return { from, to: Math.min(count, Math.ceil((at + 2 * MARGIN) / STEP) * STEP) };
}

/** The same lines, or `shown` widened just enough to keep `at` among them. */
export function keeping(shown: Shown, at: number | null): Shown {
  if (at === null) {
    return shown;
  }
  return { from: Math.min(shown.from, at), to: Math.max(shown.to, at + 1) };
}
