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
  const pieces: string[] = [];
  let piece: string[] = [];
  const step = values.length > 1 ? width / (values.length - 1) : 0;
  values.forEach((value, index) => {
    if (value === null) {
      if (piece.length > 0) {
        pieces.push(piece.join(" "));
        piece = [];
      }
      return;
    }
    const x = values.length > 1 ? index * step : width / 2;
    const y = height - (Math.min(value, ceiling) / ceiling) * height;
    piece.push(`${piece.length === 0 ? "M" : "L"}${round(x)} ${round(y)}`);
  });
  if (piece.length > 0) {
    pieces.push(piece.join(" "));
  }
  return pieces;
}

/** The same pieces closed down to the foot of the box, for the wash under the line. */
export function areaPieces(
  values: (number | null)[],
  ceiling: number,
  width: number,
  height: number,
): string[] {
  return linePieces(values, ceiling, width, height).map((piece) => {
    const points = piece.split(/[ML]/).filter(Boolean);
    const first = points[0].trim().split(" ")[0];
    const last = points[points.length - 1].trim().split(" ")[0];
    return `${piece} L${last} ${height} L${first} ${height} Z`;
  });
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
