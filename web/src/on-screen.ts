/*
 * Whether a film is on the screen.
 *
 * Said by the player of films and heard by the player of music, which stops
 * or waits while a film plays: neither of the two knows the other, and each
 * only knows this.
 */

import { useEffect, useSyncExternalStore } from "react";

let onScreen = 0;
const listening = new Set<() => void>();

function tell() {
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
