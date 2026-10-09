/*
 * Which line of stamped lyrics is being sung.
 */

import type { LyricLine } from "../api";

/** The last line whose moment has come, or -1 before the first. The lines
 *  are in the order they are sung, so the search halves the list each
 *  step: a song's words are read four times a second. */
export function lineAt(lines: readonly LyricLine[], positionMs: number): number {
  let low = 0;
  let high = lines.length - 1;
  let found = -1;
  while (low <= high) {
    const middle = (low + high) >> 1;
    if (lines[middle].at_ms <= positionMs) {
      found = middle;
      low = middle + 1;
    } else {
      high = middle - 1;
    }
  }
  return found;
}

/** How far ahead of the clock a line is lit: the time is read four times a
 *  second, and a line lit a moment late reads as one lit wrong. */
export const LIT_AHEAD_MS = 250;

/** The line that is lit when the song is this far in, in seconds. */
export function litLineAt(lines: readonly LyricLine[], positionSeconds: number): number {
  return lineAt(lines, positionSeconds * 1000 + LIT_AHEAD_MS);
}
