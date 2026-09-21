/*
 * A picture, and what to show when it does not arrive.
 *
 * Every place that shows one already knows what to draw when a work has none:
 * the letter it begins with, the number of a season. A picture the server has
 * a row for but no longer a file for is the same thing seen by somebody
 * looking at it, and without this it was not: the browser drew its own broken
 * icon over the card instead.
 *
 * It happens for real reasons. A cache emptied by hand, a cache directory
 * moved, a disk filled while the pictures were being written, and the invented
 * library the bench measures against, which has rows for its posters and
 * deliberately no files.
 */

import { useState } from "react";
import type { Picture } from "../api";
import { pictureSet } from "../api";

/** A picture to show, and what to call when the browser cannot show it. */
export interface Shown {
  picture: { src: string; srcSet: string } | null;
  /** Where a band of it is taken from when it is shown in a strip, written
      the way a stylesheet takes it. The one number every picture used before
      any of them was read, where none was. */
  framing: string;
  /** Hand to the picture's `onError`. */
  itDidNotLoad: () => void;
}

/** What a picture nobody has read is placed at: a little above the middle,
 *  which is the best one number for a picture nobody has looked at. */
const BY_DEFAULT = 0.38;

export function useShownPicture(pictures: Picture[]): Shown {
  const chosen = pictureSet(pictures);
  // The address that failed rather than a plain yes or no: the same component
  // is reused for the next work as one navigates, and a flag would carry the
  // failure of one card onto every card after it.
  const [wouldNotLoad, setWouldNotLoad] = useState<string | null>(null);

  const framing = pictures.find((picture) => picture.framing !== null)?.framing;
  return {
    picture: chosen && chosen.src !== wouldNotLoad ? chosen : null,
    framing: `center ${((framing ?? BY_DEFAULT) * 100).toFixed(1)}%`,
    itDidNotLoad: () => setWouldNotLoad(chosen?.src ?? null),
  };
}
