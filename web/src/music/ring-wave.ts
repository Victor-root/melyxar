/*
 * The outline of a wave standing round a disc, as an equaliser would stand
 * round it: a smooth closed curve through the tips of spikes growing out of
 * the disc, each as tall as it is told, and the shapes it takes one after
 * the other, for the mark of the song playing.
 */

/** A closed path in a box from -1 to 1 through the tips of as many spikes
 *  as `heights` holds, spread evenly round from the top, each standing from
 *  `inner` out to the edge of the box as its height, from nought to one,
 *  says. Each tip is a curve's corner, the way through the middle of the
 *  next, as the wave of the player is drawn. */
export function radialWave(heights: number[], inner: number): string {
  const points = heights.map((height, at) => {
    const angle = (at / heights.length) * Math.PI * 2 - Math.PI / 2;
    const radius = inner + (1 - inner) * Math.min(Math.max(height, 0), 1);
    return { x: Math.cos(angle) * radius, y: Math.sin(angle) * radius };
  });
  const middle = (a: { x: number; y: number }, b: { x: number; y: number }) =>
    `${round((a.x + b.x) / 2)} ${round((a.y + b.y) / 2)}`;
  const start = middle(points[points.length - 1], points[0]);
  const curves = points
    .map((point, at) => `Q${round(point.x)} ${round(point.y)} ${middle(point, points[(at + 1) % points.length])}`)
    .join(" ");
  return `M${start} ${curves}Z`;
}

/** `count` sets of heights for `spikes` spikes, the right half drawn at
 *  random and the left half its mirror, the same every time they are asked
 *  for: the shapes the wave goes through. */
export function mirroredHeights(spikes: number, count: number): number[][] {
  let seed = 7;
  const next = () => {
    seed = (seed * 16807) % 2147483647;
    return seed / 2147483647;
  };
  return Array.from({ length: count }, () => {
    const half = Array.from({ length: spikes / 2 + 1 }, () => 0.15 + next() * 0.85);
    return Array.from({ length: spikes }, (_, at) => half[Math.min(at, spikes - at)]);
  });
}

function round(value: number): number {
  return Math.round(value * 1000) / 1000;
}
