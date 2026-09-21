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
  /** Hand to the picture's `onError`. */
  itDidNotLoad: () => void;
}

export function useShownPicture(pictures: Picture[]): Shown {
  const chosen = pictureSet(pictures);
  // The address that failed rather than a plain yes or no: the same component
  // is reused for the next work as one navigates, and a flag would carry the
  // failure of one card onto every card after it.
  const [wouldNotLoad, setWouldNotLoad] = useState<string | null>(null);

  return {
    picture: chosen && chosen.src !== wouldNotLoad ? chosen : null,
    itDidNotLoad: () => setWouldNotLoad(chosen?.src ?? null),
  };
}
