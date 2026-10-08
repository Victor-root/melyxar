/*
 * Whether a film is on the screen.
 *
 * Said by the player of films and heard by the player of music, which stops
 * or waits while a film plays: neither of the two knows the other, and each
 * only knows this.
 *
 * Said to the stylesheet as well, by a mark on the root. Asked of the page
 * instead, with a rule for a body that has a player in it, the browser
 * checked that rule again at every change anywhere in the page and restyled
 * every card of a grid each time a card passed under the pointer: measured on
 * a library of three thousand films scrolled under a still mouse, two thirds
 * of what restyling the page cost.
 */

import { useEffect, useSyncExternalStore } from "react";

let onScreen = 0;
const listening = new Set<() => void>();

function tell() {
  if (onScreen > 0) {
    document.documentElement.dataset.filmOnScreen = "";
  } else {
    delete document.documentElement.dataset.filmOnScreen;
  }
  for (const listener of listening) {
    listener();
  }
}

/** Said by whatever plays a film, for as long as it is drawn. */
export function useFilmOnScreen(): void {
  useEffect(() => {
    onScreen += 1;
    tell();
    return () => {
      onScreen -= 1;
      tell();
    };
  }, []);
}

/** Whether a film is being played right now. */
export function useIsAFilmOnScreen(): boolean {
  return useSyncExternalStore(
    (listener) => {
      listening.add(listener);
      return () => listening.delete(listener);
    },
    () => onScreen > 0,
  );
}
