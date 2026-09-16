/*
 * The ring that turns while the server makes a film.
 *
 * Drawn to the shape Material's expressive circular indicator uses: a thin
 * neutral track, and over it an active indicator whose edge is not a smooth
 * circle but a wave running all the way round. A ring that merely spins says
 * "something is happening" and nothing else; the wave is what makes a long
 * wait read as work rather than as a hang, which is the whole reason that
 * shape exists.
 *
 * The wave is worked out here from its own figures rather than copied in as a
 * string of three thousand coordinates. Two reasons: a path nobody can read is
 * a path nobody can change, and the figures are the thing worth changing when
 * the ring is drawn at another size.
 *
 * Nothing is fetched and nothing is timed here. The turning and the sweep are
 * the stylesheet's, so they run on the compositor and cost nothing while a
 * film is being decoded beside them.
 */

/** The square the ring is drawn in, which every figure below is against. */
const BOX = 48;

/** The ring itself: how far out, how far the wave rises off it, and how many
 *  waves go round. A whole number of waves, or the wave meets itself at the
 *  top of the ring with a step in it. */
const RADIUS = 19;
const AMPLITUDE = 1.5;
const WAVES = 8;

/** How many straight pieces make one wave. Enough that no facet shows at the
 *  size the ring is drawn, and not so many the path is a wall of numbers. */
const PIECES = 16;

/** The wavy ring, and how long it is: the sweep is written in fractions of its
 *  own length, so a change to any figure above carries through on its own. */
function theWave(): { d: string; length: number } {
  const middle = BOX / 2;
  const steps = WAVES * PIECES;
  const points: [number, number][] = [];
  for (let step = 0; step <= steps; step += 1) {
    const angle = (step / steps) * 2 * Math.PI;
    const out = RADIUS + AMPLITUDE * Math.sin(WAVES * angle);
    // Begun at the top, which is where a ring is read as beginning.
    points.push([
      middle + out * Math.cos(angle - Math.PI / 2),
      middle + out * Math.sin(angle - Math.PI / 2),
    ]);
  }
  let length = 0;
  for (let at = 0; at < points.length - 1; at += 1) {
    length += Math.hypot(points[at + 1][0] - points[at][0], points[at + 1][1] - points[at][1]);
  }
  // Closed rather than ended on the point it began at, so the two ends meet
  // as one join instead of two caps sitting on each other.
  const d = `M${points
    .slice(0, -1)
    .map(([x, y]) => `${x.toFixed(2)} ${y.toFixed(2)}`)
    .join("L")}Z`;
  return { d, length };
}

const WAVE = theWave();

/**
 * Something is happening, and nobody knows how long it will take.
 *
 * Indeterminate on purpose: the server can say how much of a film it has
 * ready, but not how long the rest will take, and a bar that fills at an
 * unknown rate promises something nobody can keep.
 */
export function Spinner() {
  return (
    <svg
      className="player-spinner"
      viewBox={`0 0 ${BOX} ${BOX}`}
      role="progressbar"
      aria-valuetext=""
      focusable="false"
    >
      <circle className="player-spinner-track" cx={BOX / 2} cy={BOX / 2} r={RADIUS} />
      <path
        className="player-spinner-wave"
        d={WAVE.d}
        style={{
          // The sweep is a dash running round the ring: a piece of it lit, the
          // rest dark. Both are shares of the ring's own length so the
          // stylesheet never has to know how long it is.
          ["--player-spinner-round" as string]: `${WAVE.length.toFixed(1)}`,
        }}
      />
    </svg>
  );
}
