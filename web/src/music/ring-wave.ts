/*
 * The outline of a wave wrapped round a disc: a smooth closed curve whose
 * distance from the middle rises and falls as the wave behind the player
 * does, for the mark of the song playing.
 */

/** How many points the curve is drawn through. */
const POINTS = 96;

/** A closed path in a box from -1 to 1, its edge between `inner` and 1 away
 *  from the middle, rising and falling `lobes` times round, with a second,
 *  smaller rise laid over it so no two lobes look the same. Each point is a
 *  curve's corner, the way through the middle of the next, as the wave of
 *  the player is drawn. */
export function ringWave(lobes: number, inner: number, turn = 0): string {
  const points = Array.from({ length: POINTS }, (_, at) => {
    const angle = (at / POINTS) * Math.PI * 2;
    const swell = 0.6 * Math.sin(lobes * angle + turn) + 0.4 * Math.sin((lobes + 2) * angle + turn * 2 + 1.3);
    const radius = inner + (1 - inner) * (0.5 + swell / 2);
    return { x: Math.cos(angle) * radius, y: Math.sin(angle) * radius };
  });
  const middle = (a: { x: number; y: number }, b: { x: number; y: number }) =>
    `${round((a.x + b.x) / 2)} ${round((a.y + b.y) / 2)}`;
  const start = middle(points[POINTS - 1], points[0]);
  const curves = points
    .map((point, at) => `Q${round(point.x)} ${round(point.y)} ${middle(point, points[(at + 1) % POINTS])}`)
    .join(" ");
  return `M${start} ${curves}Z`;
}

function round(value: number): number {
  return Math.round(value * 1000) / 1000;
}
