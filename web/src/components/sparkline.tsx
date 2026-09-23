/*
 * A small curve of how a figure moved, with what it was at any point under the
 * pointer.
 *
 * Drawn by hand rather than by a charting library: a few lines of path are all
 * a curve this size is, and a library would bring a hundred kilobytes and its
 * own idea of colours. The line wears the accent, the wash under it a faint
 * share of the same, and a second line, when there is one, stays quiet in the
 * ink of the text beside it.
 */

import { useState } from "react";
import { areaPieces, ceilingOf, linePieces, pointUnder } from "../curves";

/** The box the path is laid out in; the page stretches it to fit. */
const WIDTH = 100;
const HEIGHT = 40;

export interface Series {
  values: (number | null)[];
  /** The main line wears the accent; a second one stays quiet. */
  quiet?: boolean;
}

export function Sparkline({
  series,
  ceiling,
  say,
  label,
}: {
  series: Series[];
  /** A fixed top, for a share that cannot pass one. */
  ceiling?: number;
  /** What point `index` was, for the words under the pointer. */
  say: (index: number) => string;
  /** What the curve is of, for whoever cannot see it. */
  label: string;
}) {
  const [over, setOver] = useState<number | null>(null);
  const count = Math.max(0, ...series.map((one) => one.values.length));
  const top = ceilingOf(
    series.flatMap((one) => one.values),
    ceiling,
  );

  if (count === 0) {
    return <div className="sparkline sparkline-empty" aria-label={label} />;
  }

  const at = over === null ? null : (over / Math.max(1, count - 1)) * 100;
  const main = series.find((one) => !one.quiet) ?? series[0];
  const value = over === null ? null : main.values[over];

  return (
    <div
      className="sparkline"
      role="img"
      aria-label={label}
      onPointerMove={(event) => {
        const box = event.currentTarget.getBoundingClientRect();
        setOver(pointUnder((event.clientX - box.left) / box.width, count));
      }}
      onPointerLeave={() => setOver(null)}
    >
      <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} preserveAspectRatio="none" aria-hidden="true">
        {series.map((one, index) => (
          <g key={index} className={one.quiet ? "sparkline-quiet" : "sparkline-main"}>
            {!one.quiet &&
              areaPieces(one.values, top, WIDTH, HEIGHT).map((piece) => (
                <path key={piece} className="sparkline-area" d={piece} />
              ))}
            {linePieces(one.values, top, WIDTH, HEIGHT).map((piece) => (
              <path key={piece} className="sparkline-line" d={piece} />
            ))}
          </g>
        ))}
      </svg>
      {at !== null && (
        <>
          <span className="sparkline-rule" style={{ left: `${at}%` }} aria-hidden="true" />
          {value !== null && value !== undefined && (
            <span
              className="sparkline-dot"
              style={{ left: `${at}%`, top: `${100 - (Math.min(value, top) / top) * 100}%` }}
              aria-hidden="true"
            />
          )}
          <span
            className={`sparkline-say${at > 60 ? " sparkline-say-left" : ""}`}
            style={{ left: `${at}%` }}
          >
            {say(over!)}
          </span>
        </>
      )}
    </div>
  );
}
