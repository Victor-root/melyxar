/*
 * Whether the system leaves room at the edges of this screen: the status bar
 * and the navigation bar of an installed app drawn edge to edge.
 *
 * Read from the room itself, not from how the app was installed: an installed
 * app without the edge mode leaves no room, and keeps the layout it has in a
 * browser tab. Said to the stylesheet by a mark on the root, the way the film
 * on screen is.
 */

const SIDES = ["top", "bottom", "left", "right"];

/** Whether any of the four rooms, as the browser reads them, is more than none. */
export function leavesRoom(rooms: readonly string[]): boolean {
  return rooms.some((room) => parseFloat(room) > 0);
}

/** Keeps the mark on the root up to date with whether the system leaves room. */
export function markTheEdgeRoom(): void {
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
