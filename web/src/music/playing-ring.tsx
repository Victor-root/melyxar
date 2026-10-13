/*
 * The mark of the song playing: a wave wrapped round its button, filled in
 * the accent as the wave behind the player is. Two waves of different
 * lobes, laid one over the other, turn slowly in opposite directions and
 * breathe, so the edge seems to ripple without anything being drawn again:
 * each moves by transform alone, on the one line that is playing. It
 * follows nothing of the song, and while the song is paused it draws back
 * behind the button.
 */

import { ringWave } from "./ring-wave";

/** How far in the wave starts, as a share of how far it reaches: under the
 *  button, so only its rippling edge shows round it. */
const INNER = 0.6;

const WAVES = [ringWave(5, INNER), ringWave(7, INNER, 2.1)];

export function PlayingRing({ playing }: { playing: boolean }) {
  return (
    <span className={`playing-ring${playing ? " playing-ring-on" : ""}`} aria-hidden="true">
      {WAVES.map((wave, at) => (
        <svg key={at} className="playing-ring-wave" viewBox="-1 -1 2 2">
          <path d={wave} />
        </svg>
      ))}
    </span>
  );
}
