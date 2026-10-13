/*
 * The levels of the wave behind the player, said each time it is drawn to
 * whatever moves with it: the bars inside the button of the song playing.
 * Nothing is worked out for them that the wave does not already work out.
 */

import { barsOf } from "./spectrum-data";

type Hearing = (bars: readonly number[]) => void;

const hearing = new Set<Hearing>();

/** Said by the wave as it is drawn; nothing is done while nobody listens. */
export function sayWaveLevels(levels: readonly number[]): void {
  if (hearing.size === 0) {
    return;
  }
  const bars = barsOf(levels);
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
