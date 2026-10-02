/*
 * A small filled wave in the accent, like the one behind the player, the mark
 * of the song that is playing: it drifts and swells at random while it plays
 * and lies low when it is paused.
 */

/** The width of the picture, which is also how far a wave drifts before it
 *  looks the same again. */
const WIDTH = 22;
const HEIGHT = 16;

/** How high the wave stands at each point along one width of it. */
const LEVELS = [11, 0, 9, 1, 13];

/**
 * A wave laid over three widths, from one before the picture to one after
 * it, so that drifting it by one width never shows an end. Smoothed the way
 * the wave of the player is: each point a curve's corner, its way through
 * the middle of the next.
 */
function wave(levels: number[]): string {
  const step = WIDTH / levels.length;
  const points = Array.from({ length: levels.length * 4 + 1 }, (_, index) => ({
    x: (index - levels.length) * step,
    y: levels[index % levels.length],
  }));
  const curves = points
    .slice(0, -1)
    .map((point, index) => {
      const next = points[index + 1];
      return `Q${point.x} ${point.y} ${(point.x + next.x) / 2} ${(point.y + next.y) / 2}`;
    })
    .join(" ");
  const last = points[points.length - 1];
  return `M${points[0].x} ${HEIGHT} L${points[0].x} ${points[0].y} ${curves} L${last.x} ${last.y} L${last.x} ${HEIGHT}Z`;
}

const WAVE = wave(LEVELS);

export function PlayingWave({ playing }: { playing: boolean }) {
  return (
    <span className={`playing-wave${playing ? " playing-wave-on" : ""}`} aria-hidden="true">
      <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`}>
        <g className="playing-wave-swell">
          <path className="playing-wave-body" d={WAVE} />
        </g>
      </svg>
    </span>
  );
}
