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
