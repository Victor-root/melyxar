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
 * under the system's bars and is no installed app to the browser inside it. A
 * browser tab keeps its layout whatever the browser says of the edges.
 */

const SIDES = ["top", "bottom", "left", "right"];

/** What Melyxar's Android application adds to the name its pages give. */
const IN_THE_APP = / MelyxarApp\//;

/** Whether the page has the whole screen, the system's bars drawn over it. */
export function hasTheWholeScreen(installed: boolean, userAgent: string): boolean {
  return installed || IN_THE_APP.test(userAgent);
}

/** Whether any of the four rooms, as the browser reads them, is more than none. */
export function leavesRoom(rooms: readonly string[]): boolean {
  return rooms.some((room) => parseFloat(room) > 0);
}

/** Keeps the marks on the root up to date with whether the page has the whole
 *  screen, and whether the system leaves room at its edges. */
export function markTheEdgeRoom(): void {
  const installed = window.matchMedia("(display-mode: standalone)");
  const whole = () => {
    if (hasTheWholeScreen(installed.matches, navigator.userAgent)) {
      document.documentElement.dataset.wholeScreen = "";
    } else {
      delete document.documentElement.dataset.wholeScreen;
    }
  };
  whole();
  installed.addEventListener("change", whole);

  const probe = document.createElement("div");
  probe.style.cssText = `position: fixed; visibility: hidden; pointer-events: none; ${SIDES.map(
    (side) => `padding-${side}: env(safe-area-inset-${side}, 0px)`,
  ).join("; ")}`;
  document.body.append(probe);

  const tell = () => {
    const style = getComputedStyle(probe);
    const rooms = SIDES.map((side) => style.getPropertyValue(`padding-${side}`));
    if (leavesRoom(rooms)) {
      document.documentElement.dataset.edgeRoom = "";
    } else {
      delete document.documentElement.dataset.edgeRoom;
    }
  };
  tell();
  window.addEventListener("resize", tell);
}
