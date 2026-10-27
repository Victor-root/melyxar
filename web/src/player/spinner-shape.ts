/*
 * The figures of the loading ring, as Material's expressive circular
 * indicator draws it (`CircularWavyProgressIndicator`, the ring Omnify shows
 * on its first load).
 *
 * Its wavy edge is not a sine but a rounded star, made by the shapes library
 * of Android: `RoundedPolygon.star(13, innerRadius = 0.75, rounding = 0.35
 * smoothed 0.4, innerRounding = 0.5)`, normalised to a square of one. Thirteen
 * points because the ring is 64 across with a stroke of 4, and Material puts
 * one wave every 15 along a circle of radius 30. The curves of one wave
 * below are read out of that library, centred on the middle of the square:
 * the ring is those, turned round thirteen times.
 *
 * The flat ring the wave grows out of, at the start and the end of a
 * percentage, is the same curves pulled out onto a circle, so the browser can
 * blend one into the other point by point.
 */

/** The square the ring is drawn in. */
export const BOX = 64;
/** The width of both lines, the lit one and the track. */
export const STROKE = 4;
/** The space Material leaves between the lit line and the track, before
 *  the round ends of both are counted. */
const GAP = 4;
/** How many waves go round. */
const WAVES = 13;
/** The radius of the plain circle the track is drawn on, in a square of one. */
const ROUND = 0.493962;

type Cubic = [number, number, number, number, number, number, number, number];

/** One wave: from its crest on the angle nought down into the bend, round
 *  the bend, and up to the next crest, as the library lays them. */
const DOWN: Cubic = [
  0.491694, 0, 0.491694, 0.019832, 0.482374, 0.039663, 0.463733, 0.051974,
];
const BEND: Cubic = [
  0.463733, 0.051974, 0.425022, 0.077537, 0.412448, 0.128553, 0.434844, 0.169179,
];
/** The way up to a crest, laid against the crest on the angle nought. */
const UP: Cubic = [
  0.463733, -0.051973, 0.482374, -0.039663, 0.491694, -0.019832, 0.491694, 0,
];

const SCALE = BOX - STROKE;
const MIDDLE = BOX / 2;

/** The radius the track is drawn on, in the square of the ring. */
export const TRACK_RADIUS = ROUND * SCALE;
/** The track's length, which the percentages along it are shares of. */
export const TRACK_LENGTH = 2 * Math.PI * TRACK_RADIUS;

function turned(cubic: Cubic, angle: number): Cubic {
  const [cos, sin] = [Math.cos(angle), Math.sin(angle)];
  const out = [...cubic] as Cubic;
  for (let at = 0; at < 8; at += 2) {
    out[at] = cubic[at] * cos - cubic[at + 1] * sin;
    out[at + 1] = cubic[at] * sin + cubic[at + 1] * cos;
  }
  return out;
}

/** The wavy ring as curves, begun at the crest on the angle nought and going
 *  round clockwise, as Material's begins. */
function theStar(): Cubic[] {
  const step = (2 * Math.PI) / WAVES;
  const curves: Cubic[] = [];
  for (let wave = 0; wave < WAVES; wave += 1) {
    curves.push(turned(DOWN, wave * step), turned(BEND, wave * step), turned(UP, (wave + 1) * step));
  }
  return curves;
}

/** The same curves, every point pulled out onto the circle at its angle. */
function flattened(curves: Cubic[]): Cubic[] {
  return curves.map((cubic) => {
    const out = [...cubic] as Cubic;
    for (let at = 0; at < 8; at += 2) {
      const angle = Math.atan2(cubic[at + 1], cubic[at]);
      out[at] = ROUND * Math.cos(angle);
      out[at + 1] = ROUND * Math.sin(angle);
    }
    return out;
  });
}

const placed = (value: number) => (MIDDLE + value * SCALE).toFixed(3);

function asPath(curves: Cubic[]): string {
  const [x, y] = curves[0];
  return (
    `M${placed(x)} ${placed(y)}` +
    curves
      .map(([, , x1, y1, x2, y2, x3, y3]) => `C${[x1, y1, x2, y2, x3, y3].map(placed).join(" ")}`)
      .join("") +
    "Z"
  );
}

/** How long a set of curves is, in the square of the ring. */
function lengthOf(curves: Cubic[]): number {
  let length = 0;
  for (const [x0, y0, x1, y1, x2, y2, x3, y3] of curves) {
    let [px, py] = [x0, y0];
    for (let step = 1; step <= 32; step += 1) {
      const t = step / 32;
      const u = 1 - t;
      const x = u * u * u * x0 + 3 * u * u * t * x1 + 3 * u * t * t * x2 + t * t * t * x3;
      const y = u * u * u * y0 + 3 * u * u * t * y1 + 3 * u * t * t * y2 + t * t * t * y3;
      length += Math.hypot(x - px, y - py);
      [px, py] = [x, y];
    }
  }
  return length * SCALE;
}

const STAR = theStar();
const FLAT = flattened(STAR);

/** The wavy ring, and the flat one it grows out of, as SVG paths. */
export const WAVY_PATH = asPath(STAR);
export const FLAT_PATH = asPath(FLAT);
const WAVY_LENGTH = lengthOf(STAR);
const FLAT_LENGTH = lengthOf(FLAT);

/** Whether the wave shows at a percentage: Material keeps the ring flat up to
 *  a tenth and from ninety-five on, so it starts and ends as a plain line. */
export function wavyAt(percent: number): boolean {
  return percent > 10 && percent < 95;
}

/**
 * The track left over by a lit share, as a dash along a track a hundred long:
 * how long it is and where it begins, or nothing when no track shows. Away
 * from both ends of the lit line by the gap and by the round ends of the two
 * lines, but never by more than the lit line is long, so an empty ring is a
 * whole track.
 */
export function trackAround(percent: number, wavy: boolean): { length: number; from: number } | null {
  const lit = Math.min(100, Math.max(0, percent));
  const litLength = (lit / 100) * (wavy ? WAVY_LENGTH : FLAT_LENGTH);
  const apart = ((Math.min(litLength, GAP) + 2 * Math.min(litLength, STROKE / 2)) / TRACK_LENGTH) * 100;
  const from = lit + apart;
  const length = 100 - apart - from;
  return length > 0 ? { length, from } : null;
}
