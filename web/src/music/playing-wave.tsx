/*
 * A small wave in the accent, the mark of the song that is playing: it
 * drifts and swells at random while it plays and lies low when it is paused.
 */

/** One hump up and one down, laid end to end past the edge of the picture. */
const WAVE = "M0 8 Q2.75 0 5.5 8 T11 8 T16.5 8 T22 8 T27.5 8 T33 8";

export function PlayingWave({ playing }: { playing: boolean }) {
  return (
    <span className={`playing-wave${playing ? " playing-wave-on" : ""}`} aria-hidden="true">
      <svg viewBox="0 0 22 16">
        <g className="playing-wave-swell playing-wave-swell-front">
          <path className="playing-wave-line playing-wave-line-front" d={WAVE} />
        </g>
        <g className="playing-wave-swell playing-wave-swell-back">
          <path className="playing-wave-line playing-wave-line-back" d={WAVE} />
        </g>
      </svg>
    </span>
  );
}
