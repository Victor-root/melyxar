/*
 * The colours of the count on the bell: one colour for what waits when all of
 * it is of one kind, and one slice of the same size for each kind when it is
 * not, so the eye reads at once what mix is waiting.
 */

import type { Level } from "./api";

/** The order the colours are laid in, the gravest last. */
export const GRAVITY: Level[] = ["ok", "news", "attention", "trouble"];

/** The kinds present among some levels, in the order of the slices. */
export function kindsOf(levels: Level[]): Level[] {
  return GRAVITY.filter((level) => levels.includes(level));
}

/** The background of the count: a single colour, or equal slices from the top
 *  going round. Nothing for nothing waiting. */
export function pieOf(levels: Level[]): string | undefined {
  const kinds = kindsOf(levels);
  if (kinds.length === 0) {
    return undefined;
  }
  if (kinds.length === 1) {
    return `var(--${kinds[0]})`;
  }
  const slices = kinds.map(
    (kind, at) => `var(--${kind}) ${(at / kinds.length) * 100}% ${((at + 1) / kinds.length) * 100}%`,
  );
  return `conic-gradient(${slices.join(", ")})`;
}
