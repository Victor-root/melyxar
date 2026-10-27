/*
 * The ring that turns while the server makes a film.
 *
 * Material's expressive circular indicator, the one Omnify shows on its first
 * load, to the figure (see `spinner-shape.ts` for its shape): a lit line with
 * a wavy edge, the wave running along it at one wave a second, and a track
 * finishing the circle with a gap at each end.
 *
 * Nothing is fetched and nothing is timed here. The turning, the sweep and
 * the running wave are the stylesheet's, so nothing runs in script while a
 * film is being decoded beside them.
 */

import type { CSSProperties } from "react";

import { BOX, FLAT_PATH, TRACK_RADIUS, WAVY_PATH, trackAround, wavyAt } from "./spinner-shape";

/** A path given as a style rather than an attribute, so a change of shape is
 *  a transition the browser blends rather than a jump. */
const drawn = (path: string) => ({ d: `path("${path}")` }) as CSSProperties;

/**
 * Something is happening. Without a `percent`, nobody knows how long it will
 * take and the ring turns. With one, the line is lit that far round from the
 * top, and goes no further than what was measured.
 */
export function Spinner({ percent }: { percent?: number }) {
  const known = percent !== undefined;
  const lit = known ? Math.min(100, Math.max(0, percent)) : 0;
  const wavy = !known || wavyAt(lit);
  const track = known ? trackAround(lit, wavy) : null;
  const middle = BOX / 2;
  return (
    <svg
      className={`player-spinner${known ? " player-spinner-known" : ""}${wavy ? "" : " player-spinner-flat"}`}
      viewBox={`0 0 ${BOX} ${BOX}`}
      role="progressbar"
      aria-valuemin={known ? 0 : undefined}
      aria-valuemax={known ? 100 : undefined}
      aria-valuenow={known ? Math.round(lit) : undefined}
      aria-valuetext={known ? undefined : ""}
      focusable="false"
    >
      <g className="player-spinner-turn">
        <g className="player-spinner-step">
          {(!known || track) && (
            <circle
              className="player-spinner-track"
              cx={middle}
              cy={middle}
              r={TRACK_RADIUS}
              pathLength={100}
              style={
                track ? { strokeDasharray: `${track.length} ${100 - track.length}`, strokeDashoffset: -track.from } : undefined
              }
            />
          )}
          {(!known || lit > 0) && (
            <path
              className="player-spinner-wave"
              d={wavy ? WAVY_PATH : FLAT_PATH}
              pathLength={100}
              style={{
                ...drawn(wavy ? WAVY_PATH : FLAT_PATH),
                ...(known ? { strokeDasharray: `${lit} ${100 - lit}` } : {}),
              }}
            />
          )}
        </g>
      </g>
    </svg>
  );
}
