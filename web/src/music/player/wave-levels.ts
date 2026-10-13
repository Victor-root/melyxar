/*
 * The levels of the wave behind the player, said each time it is drawn to
 * whatever moves with it: the bars inside the button of the song playing.
 * Only the levels the wave reads are said, and the bars are worked out from
 * them here, once for every button showing them.
 */

import { BarMeter, barsOf } from "./spectrum-data";

type Hearing = (bars: readonly number[]) => void;

const hearing = new Set<Hearing>();
const meter = new BarMeter();
let lastSaid = 0;

/** Said by the wave as it is drawn, with the levels it is heading for;
 *  nothing is done while nobody listens. */
export function sayWaveLevels(levels: readonly number[], now: number): void {
  const since = lastSaid === 0 ? 0 : now - lastSaid;
  lastSaid = now;
  if (hearing.size === 0) {
    return;
  }
  const bars = meter.step(barsOf(levels), since);
  for (const one of hearing) {
    one(bars);
  }
}

/** Listens to the wave as it is drawn, until the returned call. */
export function hearWaveLevels(one: Hearing): () => void {
  hearing.add(one);
  return () => {
    hearing.delete(one);
  };
}
