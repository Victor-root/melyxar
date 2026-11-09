/*
 * What is drawn behind a page, as the account chose it: one of the paintings
 * of light, the drawn shelf of the door, or nothing over the plain surface of
 * the theme.
 *
 * Fixed to the window rather than to the page, which styles/pages.css says:
 * each page that wears it asks for it, and it is the same painting on all of
 * them. So it is drawn once for the window and stays where it is from one
 * page to the next. Drawn by each page, it went with the page it was in: on
 * every change of page it vanished, was painted again and faded in over a
 * second, across the whole screen, while the new page was being built.
 */

import { useLayoutEffect, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import { useSettings } from "../settings";
import { DrawnLibrary } from "./door-background";

/* How many pages on screen ask for the window's backdrop. A page left and
   the next one arriving are one commit, so the count goes down and up again
   before anything is drawn, and the painting is never taken away between. */
let asking = 0;
const following = new Set<() => void>();

function follow(heard: () => void) {
  following.add(heard);
  return () => {
    following.delete(heard);
  };
}

function askedFor() {
  return asking > 0;
}

function moved(by: number) {
  asking += by;
  for (const heard of following) heard();
}

function Drawn() {
  const { backdrop, backdropLight } = useSettings();
  if (backdrop === "none") {
    return null;
  }
  return backdrop === "library" ? (
    <div className="home-backdrop" aria-hidden="true">
      <DrawnLibrary />
    </div>
  ) : (
    <div className="home-backdrop drift" data-light={backdropLight} aria-hidden="true" />
  );
}

/** Asks for the window's backdrop for as long as the page is shown. A page
 *  that covers the whole window draws its own, in place, under its own
 *  words. */
export function PageBackdrop({ inPlace = false }: { inPlace?: boolean }) {
  useLayoutEffect(() => {
    if (inPlace) {
      return;
    }
    moved(1);
    return () => moved(-1);
  }, [inPlace]);
  return inPlace ? <Drawn /> : null;
}

/** The window's backdrop, while a page asks for it. Put on the body rather
 *  than in the page: what stands over the page, such as the fade of the page
 *  into the player of music, must never take it along. */
export function WindowBackdrop() {
  const shown = useSyncExternalStore(follow, askedFor);
  return shown ? createPortal(<Drawn />, document.body) : null;
}
