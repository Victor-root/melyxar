/*
 * Where a window standing on the picture goes when a hand moves it by its
 * title bar or pulls one of its edges.
 *
 * Worked out from where it stood when the hand took hold and how far the
 * hand has gone since, so nothing is measured while it moves. It never
 * leaves the picture, and never shrinks past the size its words still read
 * at.
 */

/** A window's place and size, in pixels from the top left of the picture. */
export interface Frame {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** What the hand holds: the title bar, or an edge or corner named by the
 *  sides of the window it moves. */
export type Hold = "move" | "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";

/** The smallest a window is pulled down to. */
export const SMALLEST = { width: 240, height: 160 };

function kept(value: number, low: number, high: number): number {
  return Math.min(Math.max(value, low), high);
}

/** Where the window stands once the hand has moved by `across` and `down`
 *  from where it took hold. */
export function framed(
  hold: Hold,
  from: Frame,
  room: { width: number; height: number },
  across: number,
  down: number,
): Frame {
  if (hold === "move") {
    return {
      ...from,
      left: kept(from.left + across, 0, room.width - from.width),
      top: kept(from.top + down, 0, room.height - from.height),
    };
  }

  let { left, top, width, height } = from;
  const right = from.left + from.width;
  const bottom = from.top + from.height;
  if (hold.includes("e")) {
    width = kept(from.width + across, SMALLEST.width, room.width - from.left);
  }
  if (hold.includes("w")) {
    left = kept(from.left + across, 0, right - SMALLEST.width);
    width = right - left;
  }
  if (hold.includes("s")) {
    height = kept(from.height + down, SMALLEST.height, room.height - from.top);
  }
  if (hold.includes("n")) {
    top = kept(from.top + down, 0, bottom - SMALLEST.height);
    height = bottom - top;
  }
  return { left, top, width, height };
}
