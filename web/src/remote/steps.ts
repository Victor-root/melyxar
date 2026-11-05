/*
 * How far one step of volume goes on a page, the remote's minus and plus:
 * a television goes by its own steps, as its keys do.
 */

/** One step, a twentieth of the whole. */
const ONE_STEP = 0.05;

/** The volume moved by so many steps, never below nothing nor above
 *  everything, and landing on a step whatever it started from. */
export function steppedVolume(volume: number, steps: number): number {
  const landed = Math.round((volume + steps * ONE_STEP) / ONE_STEP) * ONE_STEP;
  return Math.min(1, Math.max(0, Number(landed.toFixed(2))));
}
