/*
 * The mark of the song playing: its red button itself rippling. While the
 * song plays, the disc of the button gives way to two waves of the same
 * accent, of different lobes, laid one over the other, which turn slowly in
 * opposite directions and breathe: the outline they make together keeps
 * changing, so the button seems to ripple at its edge without anything being
 * drawn again, each moving by transform alone, on the one line that is
 * playing. It follows nothing of the song; paused, the button is a plain
 * disc again.
 */

import { ringWave } from "./ring-wave";

/** How far in the edge of the wave can come, as a share of how far it
 *  reaches: shallow, so the button keeps its size and only its edge moves. */
const INNER = 0.8;

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
