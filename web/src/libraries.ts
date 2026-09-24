/*
 * The libraries the whole interface is built from, held in one place.
 *
 * The navigation, the home page, the search and the activity screen are all
 * drawn from this one list, and the settings screen is where it changes.
 * Fetched once per screen, a library declared or taken away would leave the
 * bar at the top listing something that is no longer there until somebody
 * reloaded the page, which is exactly what this interface promises never to
 * make anyone do.
 *
 * So it is read here, and read again whenever something changed it: at once
 * when this interface changed it, and whenever work ends, since a scan is what
 * makes a library grow.
 */

import { createContext, useContext } from "react";
import { api } from "./api";
import { useAsked } from "./asking";
import type { Library, LibraryKind } from "./api";
import type { CardShape } from "./components/card";
import type { Wording } from "./readable";

export interface Libraries {
  all: Library[];
  /** Told to look again now, after declaring one or taking one away. */
  refresh: () => void;
}

/** Every kind a library can be, in the order they are offered and read in:
 *  the choice when one is declared, the categories of the bar at the top, the
 *  kinds a search may be narrowed to. */
export const KINDS: LibraryKind[] = ["movies", "series", "anime", "home_media", "shows", "music"];

export const LibrariesContext = createContext<Libraries>({
  all: [],
  refresh: () => {},
});

/**
 * What the provider at the top of the interface holds.
 *
 * Reads again each time work ends, which is how the count beside a library
 * grows during a scan without anybody asking.
 */
export function useWatchedLibraries(finished: number): Libraries {
  // A server that did not answer keeps the list that was there, which is what
  // asking is built to do: emptying the navigation over one failed request
  // would say the collection is gone, a far worse thing to say than nothing.
  const asked = useAsked((signal) => api.libraries(signal), [finished]);
  return { all: asked.answer ?? [], refresh: asked.again };
}

/** The libraries, for any page or part of the bar that shows them. */
export function useLibraries(): Libraries {
  return useContext(LibrariesContext);
}

/** The libraries of one kind, which is what a category really is. */
export function librariesOfKind(kind: LibraryKind, libraries: Library[]): Library[] {
  return libraries.filter((library) => library.kind === kind);
}

/**
 * What a category is called: the name the administrator gave its library when
 * it holds only one, since the category is then that library, and the kind's
 * own name when it gathers several.
 */
export function nameOfKind(kind: LibraryKind, libraries: Library[], t: Wording): string {
  const ofThatKind = librariesOfKind(kind, libraries);
  return ofThatKind.length === 1 ? ofThatKind[0].name : t(`kind.${kind}`);
}

/** The title of the home page's row of what came in last in a category, named
 *  like the category itself. */
export function newestOfKind(kind: LibraryKind, libraries: Library[], t: Wording): string {
  const ofThatKind = librariesOfKind(kind, libraries);
  return ofThatKind.length === 1
    ? t("home.newest.named", { name: ofThatKind[0].name })
    : t(`home.newest.${kind}`);
}

/**
 * Where a whole category leads.
 *
 * Straight into the library when a kind holds one, since a grid of everything
 * of that kind and the grid of that library are then the same page. Into a
 * grid narrowed to the kind when it holds several, because a collection of
 * films spread over four disks is one category and nobody wants to be asked
 * which disk they meant.
 */
export function whereAKindLeads(kind: LibraryKind, libraries: Library[]): string {
  const ofThatKind = librariesOfKind(kind, libraries);
  return ofThatKind.length === 1 ? `/library/${ofThatKind[0].id}` : `/search?in=kind:${kind}`;
}

/**
 * The kinds the home page gives a tile and a row, in the order this account
 * chose. Music has neither yet, on the server as here.
 */
export function kindsOnTheHomePage(order: LibraryKind[], libraries: Library[]): LibraryKind[] {
  return order.filter(
    (kind) => kind !== "music" && libraries.some((library) => library.kind === kind),
  );
}

/**
 * The order with one kind swapped with its neighbour among those shown. The
 * kinds nobody sees keep their places, so a kind that comes back later finds
 * the place it had.
 */
export function movedOnTheHomePage(
  order: LibraryKind[],
  shown: LibraryKind[],
  kind: LibraryKind,
  step: -1 | 1,
): LibraryKind[] {
  const from = shown.indexOf(kind);
  const to = from + step;
  if (from < 0 || to < 0 || to >= shown.length) {
    return order;
  }
  const swapped = [...shown];
  [swapped[from], swapped[to]] = [swapped[to], swapped[from]];
  let next = 0;
  return order.map((one) => (shown.includes(one) ? swapped[next++] : one));
}

/** How the cards of a kind are laid out: on their side for what somebody
 *  filmed and photographed themselves, which is mostly wider than tall and
 *  has no poster standing up; standing for everything else. */
export function cardShapeOf(kind: LibraryKind | undefined): CardShape {
  return kind === "home_media" ? "lying" : "standing";
}
