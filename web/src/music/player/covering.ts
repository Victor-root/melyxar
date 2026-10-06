/*
 * Whether the page of what is playing covers the whole screen, as it does on
 * a phone once it has faded in.
 *
 * Everything under it is then out of sight, and is told so: the root carries
 * a mark the stylesheet hides the rest of the interface by, so the browser
 * stops drawing and composing it, and the wave of the bar stops drawing under
 * a page that hides it.
 */

import { useSyncExternalStore } from "react";

const listeners = new Set<() => void>();
let covers = false;

export function setNowPlayingCovers(next: boolean) {
  if (next === covers) {
    return;
  }
  covers = next;
  if (next) {
    document.documentElement.dataset.nowPlayingCovers = "";
  } else {
    delete document.documentElement.dataset.nowPlayingCovers;
  }
  for (const listener of listeners) {
    listener();
  }
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useNowPlayingCovers(): boolean {
  return useSyncExternalStore(subscribe, () => covers);
}
