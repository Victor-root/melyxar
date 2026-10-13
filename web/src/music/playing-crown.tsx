/*
 * A crown of short spikes round the button of the song that is playing, the
 * mark of that song: each spike grows out of the disc and draws back on a
 * beat of its own, so the crown looks like a ring of sound without ever
 * following the song itself. The two halves mirror one another. It draws
 * back into the disc while the song is paused.
 *
 * Every spike moves by transform alone, on the one line that is playing.
 */

/** How many spikes stand round the disc. */
const SPIKES = 20;

/** A beat and a start for each spike of one half, the other half mirroring
 *  it: spread out by steps that never fall back in line, so the crown does
 *  not look as if it repeated. */
const BEATS = Array.from({ length: SPIKES / 2 + 1 }, (_, rank) => ({
  duration: 620 + ((rank * 233) % 520),
  delay: -((rank * 389) % 1100),
}));

const SPIKE_STYLES = Array.from({ length: SPIKES }, (_, at) => {
  const beat = BEATS[Math.min(at, SPIKES - at)];
  return {
    arm: { transform: `rotate(${(at * 360) / SPIKES}deg)` },
    spike: { animationDuration: `${beat.duration}ms`, animationDelay: `${beat.delay}ms` },
  };
});

export function PlayingCrown({ playing }: { playing: boolean }) {
  return (
    <span className={`playing-crown${playing ? " playing-crown-on" : ""}`} aria-hidden="true">
      {SPIKE_STYLES.map((style, at) => (
        <span key={at} className="playing-crown-arm" style={style.arm}>
          <span className="playing-crown-spike" style={style.spike} />
        </span>
      ))}
    </span>
  );
}
