/*
 * Where a picture sits in a square frame, and which square of it that shows.
 *
 * The picture always covers the whole frame: at the least zoom its short side
 * fills it exactly, and it can never be pulled far enough to show an empty
 * corner. Everything is measured in points of the frame, and only turned into
 * pixels of the picture at the end, for the square that is sent.
 */

/** How far in the picture can be brought: four times is a face out of a
 *  group photo, and further only shows its pixels. */
export const MOST_ZOOM = 4;

/** How the picture sits: how far it is zoomed, and where its top left corner
 *  is from the top left of the frame. */
export interface Framing {
  zoom: number;
  x: number;
  y: number;
}

/** The part of the picture the frame shows, in pixels of the picture. */
export interface Square {
  x: number;
  y: number;
  side: number;
}

function between(value: number, low: number, high: number): number {
  return Math.min(Math.max(value, low), high);
}

/** How many points of the frame a pixel of the picture takes at this zoom. */
function scaleOf(zoom: number, frame: number, width: number, height: number): number {
  return (frame / Math.min(width, height)) * zoom;
}

/** Keeps the picture over the whole frame, and the zoom where it may be. */
export function held(framing: Framing, frame: number, width: number, height: number): Framing {
  const zoom = between(framing.zoom, 1, MOST_ZOOM);
  const scale = scaleOf(zoom, frame, width, height);
  return {
    zoom,
    x: between(framing.x, frame - width * scale, 0),
    y: between(framing.y, frame - height * scale, 0),
  };
}

/** The picture as it first appears: its middle in the middle of the frame. */
export function centred(frame: number, width: number, height: number): Framing {
  const scale = scaleOf(1, frame, width, height);
  return { zoom: 1, x: (frame - width * scale) / 2, y: (frame - height * scale) / 2 };
}

/** Pulled along by a hand. */
export function moved(
  framing: Framing,
  across: number,
  down: number,
  frame: number,
  width: number,
  height: number,
): Framing {
  return held({ ...framing, x: framing.x + across, y: framing.y + down }, frame, width, height);
}

/** Zoomed around the middle of the frame, so what is looked at stays where
 *  it is rather than sliding off towards a corner. */
export function zoomedTo(
  framing: Framing,
  zoom: number,
  frame: number,
  width: number,
  height: number,
): Framing {
  const wanted = between(zoom, 1, MOST_ZOOM);
  const growth =
    scaleOf(wanted, frame, width, height) / scaleOf(framing.zoom, frame, width, height);
  const middle = frame / 2;
  return held(
    {
      zoom: wanted,
      x: middle - (middle - framing.x) * growth,
      y: middle - (middle - framing.y) * growth,
    },
    frame,
    width,
    height,
  );
}

/** The square of the picture the frame shows. */
export function squareShown(framing: Framing, frame: number, width: number, height: number): Square {
  const scale = scaleOf(framing.zoom, frame, width, height);
  // A held picture never starts right of or below the corner of the frame,
  // so how far it reaches past that corner is all there is to measure.
  return { x: Math.abs(framing.x) / scale, y: Math.abs(framing.y) / scale, side: frame / scale };
}
