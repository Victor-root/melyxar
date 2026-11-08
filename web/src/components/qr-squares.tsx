/*
 * A QR code as the server made it, a row of squares at a time, drawn as one
 * shape: dark squares on a light ground with the quiet margin a reader needs
 * around them, whatever the theme, since a camera reads contrast and not
 * colours. Its size is the stylesheet's, so it can follow the room.
 */

/** The squares of empty margin a reader needs around the code. */
const MARGIN = 4;

/** The dark squares as one path, a run of them in a row at a time. */
export function pathOf(squares: string[]): string {
  const parts: string[] = [];
  squares.forEach((row, y) => {
    let x = 0;
    while (x < row.length) {
      if (row[x] !== "1") {
        x += 1;
        continue;
      }
      const start = x;
      while (x < row.length && row[x] === "1") x += 1;
      parts.push(`M${start + MARGIN} ${y + MARGIN}h${x - start}v1h${start - x}z`);
    }
  });
  return parts.join("");
}

export function QrSquares({ squares, label }: { squares: string[]; label: string }) {
  const across = squares.length + 2 * MARGIN;
  return (
    <svg
      className="qr-squares"
      viewBox={`0 0 ${across} ${across}`}
      shapeRendering="crispEdges"
      role="img"
      aria-label={label}
    >
      <rect width={across} height={across} className="qr-ground" />
      <path d={pathOf(squares)} className="qr-dark" />
    </svg>
  );
}
