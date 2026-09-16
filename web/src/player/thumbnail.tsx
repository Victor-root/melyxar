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

/** How tall a thumbnail is at a given width, keeping the film's own shape. */
export function heightAt(thumbnails: PlaybackThumbnails, across: number): number {
  return thumbnails.width > 0 ? (across * thumbnails.height) / thumbnails.width : 0;
}

/**
 * The thumbnail covering one moment, drawn at the width asked for.
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
}: {
  thumbnails: PlaybackThumbnails | null;
  seconds: number;
  /** How wide to draw it. The height follows from the film's own shape. */
  across: number;
  className: string;
}) {
  const spot = thumbnails ? spotOf(thumbnails, seconds) : null;
  if (!thumbnails || !spot || across <= 0) {
    return null;
  }
  const down = heightAt(thumbnails, across);
  return (
    <span
      className={className}
      style={{
        width: `${across}px`,
        height: `${down}px`,
        backgroundImage: `url(${thumbnails.url}/${spot.sheet}.jpg)`,
        backgroundSize: `${thumbnails.columns * across}px ${thumbnails.rows * down}px`,
        backgroundPosition: `-${spot.column * across}px -${spot.row * down}px`,
      }}
    />
  );
}
