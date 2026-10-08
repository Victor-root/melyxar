/*
 * The sums behind a small curve: where each point lands in the box it is
 * drawn in, and where the line has to break.
 *
 * A stretch where nothing was measured, the server being down, is a gap in the
 * line rather than a slope across it: a line drawn straight over a night the
 * server was off says it was busy all night at the average of both ends.
 */

/** How tall a curve reaches: a fixed ceiling when there is one, such as a
 *  share that cannot pass one, or a little above the highest point. */
export function ceilingOf(values: (number | null)[], fixed?: number): number {
  if (fixed !== undefined) {
    return fixed;
  }
  const highest = Math.max(0, ...values.filter((value): value is number => value !== null));
  return highest > 0 ? highest * 1.15 : 1;
}

/** One unbroken run of a line: its path, and where it begins and ends across
 *  the box, which is what the wash under it is closed down from. */
interface Run {
  path: string;
  from: number;
  to: number;
}

function runsOf(values: (number | null)[], ceiling: number, width: number, height: number): Run[] {
  const runs: Run[] = [];
  let piece: string[] = [];
  let from = 0;
  let to = 0;
  const step = values.length > 1 ? width / (values.length - 1) : 0;
  values.forEach((value, index) => {
    if (value === null) {
      if (piece.length > 0) {
        runs.push({ path: piece.join(" "), from, to });
        piece = [];
      }
      return;
    }
    const x = round(values.length > 1 ? index * step : width / 2);
    const y = round(height - (Math.min(value, ceiling) / ceiling) * height);
    if (piece.length === 0) {
      from = x;
    }
    to = x;
    piece.push(`${piece.length === 0 ? "M" : "L"}${x} ${y}`);
  });
  if (piece.length > 0) {
    runs.push({ path: piece.join(" "), from, to });
  }
  return runs;
}

/**
 * The line through the values, as the pieces of an SVG path, one piece per
 * unbroken run. The box is `width` by `height` with nought at its foot.
 */
export function linePieces(
  values: (number | null)[],
  ceiling: number,
  width: number,
  height: number,
): string[] {
  return runsOf(values, ceiling, width, height).map((run) => run.path);
}

/** The same pieces closed down to the foot of the box, for the wash under the
 *  line, from the same runs rather than read back out of the line's text. */
export function areaPieces(
  values: (number | null)[],
  ceiling: number,
  width: number,
  height: number,
): string[] {
  return runsOf(values, ceiling, width, height).map(
    (run) => `${run.path} L${run.to} ${height} L${run.from} ${height} Z`,
  );
}

/** Which point a pointer at this share of the width is over. */
export function pointUnder(share: number, count: number): number {
  if (count <= 1) {
    return 0;
  }
  return Math.min(count - 1, Math.max(0, Math.round(share * (count - 1))));
}

function round(value: number): number {
  return Math.round(value * 100) / 100;
}
