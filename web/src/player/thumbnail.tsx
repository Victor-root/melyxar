/*
 * One little picture of the film, cut out of a sheet of many.
 *
 * The server reads a film once and writes its thumbnails as sheets: a hundred
 * of them to a file, so that dragging along the bar is one request rather than
 * a hundred. Cutting one out is a background image offset by where it stands
 * on its sheet, and that arithmetic is the same wherever a thumbnail is shown:
 * under the hand on the bar, and on a chapter card.
 */

import type { PlaybackThumbnails } from "../api";
import type { Turn } from "./overlay";

/** Which thumbnail covers a moment, and where it sits on its sheet. */
export function spotOf(
  thumbnails: PlaybackThumbnails,
  seconds: number,
): { sheet: number; column: number; row: number } | null {
  const perSheet = thumbnails.columns * thumbnails.rows;
  if (thumbnails.every_seconds <= 0 || perSheet <= 0 || thumbnails.counted <= 0) {
    return null;
  }
  const index = Math.floor(Math.max(0, seconds) / thumbnails.every_seconds);
  // Never past the last one: a sheet is filled to the end with black whatever
  // the film gave, and a black square under the cursor is worse than none.
  const kept = Math.min(index, thumbnails.counted - 1);
  const onItsSheet = kept % perSheet;
  return {
    sheet: Math.floor(kept / perSheet),
    column: onItsSheet % thumbnails.columns,
    row: Math.floor(onItsSheet / thumbnails.columns),
  };
}

/**
 * The thumbnail covering one moment, as a background filling a box of any
 * size: the sheet is stretched so one thumbnail fills the box, and moved so the
 * right one is in it. For a box whose width is the stylesheet's rather than a
 * number known here, a card in a row.
 */
export function cutOut(
  thumbnails: PlaybackThumbnails,
  seconds: number,
): { backgroundImage: string; backgroundSize: string; backgroundPosition: string } | null {
  const spot = spotOf(thumbnails, seconds);
  if (!spot) {
    return null;
  }
  /* A percentage of position slides the sheet by the share of what is left
     over once the box is taken out of it, so the last column is at a hundred
     and a sheet of one column never moves. */
  const along = (at: number, of: number) => (of > 1 ? (at / (of - 1)) * 100 : 0);
  return {
    backgroundImage: `url(${thumbnails.url}/${spot.sheet}.jpg)`,
    backgroundSize: `${thumbnails.columns * 100}% ${thumbnails.rows * 100}%`,
    backgroundPosition: `${along(spot.column, thumbnails.columns)}% ${along(spot.row, thumbnails.rows)}%`,
  };
}

/** How tall a thumbnail is at a given width, keeping the film's own shape. */
export function heightAt(thumbnails: PlaybackThumbnails, across: number): number {
  return thumbnails.width > 0 ? (across * thumbnails.height) / thumbnails.width : 0;
}

/**
 * The room a thumbnail takes once turned: a quarter turn swaps its two sides,
 * a half turn keeps them.
 */
export function turnedBox(across: number, down: number, turn: Turn): { across: number; down: number } {
  return turn === 90 || turn === 270 ? { across: down, down: across } : { across, down };
}

/**
 * The thumbnail covering one moment, drawn at the width asked for, turned the
 * way the picture is.
 *
 * Nothing at all when the film has not been read for them, which is what every
 * caller wants: an empty grey box where a picture should be says the player is
 * broken, and it is not.
 */
export function Thumbnail({
  thumbnails,
  seconds,
  across,
  className,
  turn = 0,
}: {
  thumbnails: PlaybackThumbnails | null;
  seconds: number;
  /** How wide to draw it. The height follows from the film's own shape. */
  across: number;
  className: string;
  /** How far the picture on screen is turned, so the thumbnail matches it.
   *  Its long side stays as long as asked, whichever way it then lies. */
  turn?: Turn;
}) {
  const spot = thumbnails ? spotOf(thumbnails, seconds) : null;
  if (!thumbnails || !spot || across <= 0) {
    return null;
  }
  const down = heightAt(thumbnails, across);
  const cut = (
    <span
      className={className}
      style={{
        width: `${across}px`,
        height: `${down}px`,
        backgroundImage: `url(${thumbnails.url}/${spot.sheet}.jpg)`,
        backgroundSize: `${thumbnails.columns * across}px ${thumbnails.rows * down}px`,
        backgroundPosition: `-${spot.column * across}px -${spot.row * down}px`,
        ...(turn === 0 ? {} : { transform: `translate(-50%, -50%) rotate(${turn}deg)` }),
      }}
    />
  );
  if (turn === 0) {
    return cut;
  }
  /* Turned about its middle inside a box of the room it takes once turned,
     so whatever stands around it makes room for the shape really seen. */
  const room = turnedBox(across, down, turn);
  return (
    <span
      className="thumbnail-turned"
      style={{ width: `${room.across}px`, height: `${room.down}px` }}
    >
      {cut}
    </span>
  );
}
