/*
 * Whether the system leaves room at the edges of this screen: the status bar
 * and the navigation bar of an installed app drawn edge to edge.
 *
 * Read from the room itself, not from how the app was installed: an installed
 * app without the edge mode leaves no room, and keeps the layout it has in a
 * browser tab. Said to the stylesheet by a mark on the root, the way the film
 * on screen is.
 *
 * The room is only ever made for a page that has the whole screen: an
 * installed app, or Melyxar's own Android application, which draws the page
 * under the system's bars and says itself how much room they take (see
 * app-window.ts). A browser tab keeps its layout whatever the browser says of
 * the edges.
 */

import { appWindow } from "./app-window";
import type { Edges } from "./app-window";

const SIDES = ["top", "bottom", "left", "right"] as const;

/** The room at each edge, as the stylesheet writes it, from what the
 *  application says. */
export function roomsOf(edges: Edges): string[] {
  return SIDES.map((side) => `${edges[side]}px`);
}

/** Whether any of the four rooms, as the browser reads them, is more than none. */
export function leavesRoom(rooms: readonly string[]): boolean {
  return rooms.some((room) => parseFloat(room) > 0);
}

/** Keeps the marks on the root up to date with whether the page has the whole
 *  screen, and whether the system leaves room at its edges. */
export function markTheEdgeRoom(): void {
  const root = document.documentElement;
  const installed = window.matchMedia("(display-mode: standalone)");
  const whole = () => {
    if (installed.matches || appWindow) {
      root.dataset.wholeScreen = "";
    } else {
      delete root.dataset.wholeScreen;
    }
  };
  whole();
  installed.addEventListener("change", whole);

  const mark = (rooms: readonly string[]) => {
    if (leavesRoom(rooms)) {
      root.dataset.edgeRoom = "";
    } else {
      delete root.dataset.edgeRoom;
    }
  };

  // Written over what the stylesheet reads from the browser, which inside
  // the application may say nothing at all.
  if (appWindow) {
    const app = appWindow;
    const told = () => {
      const rooms = roomsOf(JSON.parse(app.edges()) as Edges);
      SIDES.forEach((side, index) => root.style.setProperty(`--safe-${side}`, rooms[index]));
      mark(rooms);
    };
    told();
    window.melyxarEdgesChanged = told;
    return;
  }

  const probe = document.createElement("div");
  probe.style.cssText = `position: fixed; visibility: hidden; pointer-events: none; ${SIDES.map(
    (side) => `padding-${side}: env(safe-area-inset-${side}, 0px)`,
  ).join("; ")}`;
  document.body.append(probe);

  const tell = () => {
    const style = getComputedStyle(probe);
    mark(SIDES.map((side) => style.getPropertyValue(`padding-${side}`)));
  };
  tell();
  window.addEventListener("resize", tell);
}
