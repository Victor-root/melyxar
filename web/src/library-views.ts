/*
 * How the grid of a library is laid out, as each account chose it for each
 * library: posters as everybody starts with, smaller posters, stills lying
 * down, or a list.
 */

import type { LibraryView } from "./api";
import type { CardShape } from "./components/card";
import type { GridShape } from "./components/grid";

/** Every layout, in the order the picker offers them. */
export const LIBRARY_VIEWS: readonly LibraryView[] = ["posters", "compact_posters", "thumbnails", "list"];

/** A layout there is, the posters for anything else. */
export function libraryViewOf(word: string | null | undefined): LibraryView {
  return LIBRARY_VIEWS.find((view) => view === word) ?? "posters";
}

/** The layouts as this browser keeps them, `library:layout` parted by
 *  commas, with what does not read as one left out. */
export function viewsRead(written: string | null): Record<string, LibraryView> {
  const views: Record<string, LibraryView> = {};
  for (const entry of (written ?? "").split(",")) {
    const [library, view] = entry.split(":");
    if (library && view && libraryViewOf(view) === view && view !== "posters") {
      views[library] = view;
    }
  }
  return views;
}

export function viewsWritten(views: Record<string, LibraryView>): string {
  return Object.entries(views)
    .map(([library, view]) => `${library}:${view}`)
    .join(",");
}

/** The layouts with one library's changed. The posters are what a library
 *  has when nothing is said, so choosing them takes the library out. */
export function withView(
  views: Record<string, LibraryView>,
  library: string,
  view: LibraryView,
): Record<string, LibraryView> {
  const changed = { ...views };
  if (view === "posters") {
    delete changed[library];
  } else {
    changed[library] = view;
  }
  return changed;
}

/** How the grid and each of its cards are drawn in a layout, for a library
 *  whose cards stand or lie by nature. */
export function layoutOf(view: LibraryView, usual: CardShape): { grid: GridShape; card: CardShape } {
  switch (view) {
    case "posters":
      return { grid: usual, card: usual };
    case "compact_posters":
      return { grid: "compact", card: "standing" };
    case "thumbnails":
      return { grid: "lying", card: "lying" };
    case "list":
      return { grid: "list", card: usual };
  }
}
