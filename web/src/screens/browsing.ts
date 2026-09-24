/*
 * What a grid of a library is driven by, and nothing about how it looks.
 *
 * The choices live in the address rather than in memory, so a sorted, filtered
 * grid can be kept as a link, reloaded, and walked back to with the browser's
 * own back button. Reading them out of the address and writing them back into
 * it is therefore part of the question, not part of the drawing.
 *
 * The grid is filled a page at a time, and the pages are added to what is
 * already there rather than replacing it: the cards on screen stay where they
 * are, which is what keeps the scroll position honest. That, and the guard
 * against asking for the next page twice, is the one piece of state here that
 * a grid drawn another way would still need exactly as it is.
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import { useLocation, useParams, useSearchParams } from "react-router-dom";
import { api } from "../api";
import type { Card, Filters, LibraryKind } from "../api";
import { useAsked, wasAbandoned } from "../asking";
import { keep, recall } from "../kept";
import { KINDS } from "../libraries";

/** The orderings a library can be read in. */
export const ORDERS = ["title", "added_at", "release_year", "community_rating", "runtime"] as const;

/** What the viewer has narrowed the library down to, as the address holds it. */
export interface Narrowing {
  library: string | undefined;
  order: string;
  descending: boolean;
  genre: string | undefined;
  decade: number | undefined;
  search: string | undefined;
  unidentified: boolean;
  initial: string | undefined;
  /** Only what this account marked. A view rather than a library, so it
      sorts, filters and pages like everything else. */
  favourites: boolean;
  /** Only the libraries of one kind, which is what the search's scope
      narrows by when it names a category rather than a folder. */
  kind: LibraryKind | undefined;
}

/**
 * What the search box put in the address, read back.
 *
 * One value rather than two, because it is one choice: a scope names a
 * category or one library, never both, and two parameters that must never
 * both be set are two parameters that will one day both be set.
 */
function scopeOf(written: string | null): {
  library: string | undefined;
  kind: LibraryKind | undefined;
} {
  if (written?.startsWith("kind:")) {
    const named = written.slice("kind:".length) as LibraryKind;
    return { library: undefined, kind: KINDS.includes(named) ? named : undefined };
  }
  if (written?.startsWith("library:")) {
    return { library: written.slice("library:".length), kind: undefined };
  }
  return { library: undefined, kind: undefined };
}

/** The cards of a grid read so far, and the choices they answer. */
interface Gathered {
  choices: string;
  cards: Card[];
  /** Where the next page starts, or nothing after the last. */
  next: string | null;
}

/** The same grid read again from the top, at least as far as it had been. */
async function gatherAgain(
  narrowing: Narrowing,
  as: number,
  signal: AbortSignal,
): Promise<Omit<Gathered, "choices">> {
  const cards: Card[] = [];
  let next: string | null = null;
  do {
    const page = await api.works({ ...narrowing, after: next ?? undefined }, signal);
    cards.push(...page.cards);
    next = page.next;
  } while (next !== null && cards.length < as);
  return { cards, next };
}

/** Everything a grid is handed to draw itself and to be driven by. */
export interface Browsing {
  /** What the address says the viewer is looking at. */
  narrowing: Narrowing;
  /** Puts one choice into the address, or takes it out. */
  choose: (name: string, value: string | null) => void;
  /** The cards gathered so far, in order. */
  cards: Card[];
  /** Whether there is another page after these. */
  more: boolean;
  /** Ask for it. Does nothing while one is already on its way. */
  loadMore: () => void;
  /** Whether a page is on its way. */
  loading: boolean;
  /** Whether the server could not be asked. */
  failed: boolean;
  /** What this library can be narrowed by, as the library really is: a genre
      leading to an empty grid reads as a fault. */
  filters: Filters | null;
}

export function useBrowsing(): Browsing {
  const { id } = useParams();
  const { pathname } = useLocation();
  const [parameters, setParameters] = useSearchParams();

  /* Pulled out one by one and watched one by one. Watching the address as a
     whole instead would be watching an object the router hands back afresh on
     every render, and a question rebuilt on every render is a question asked
     on every render, for ever. */
  const order = parameters.get("order") ?? "title";
  const descending = parameters.get("descending") === "true";
  const genre = parameters.get("genre") ?? undefined;
  const decade = parameters.get("decade") ? Number(parameters.get("decade")) : undefined;
  const search = parameters.get("search") ?? undefined;
  const unidentified = parameters.get("unidentified") === "true";
  const initial = parameters.get("initial") ?? undefined;
  /* Read from the path rather than from a parameter: the favourites are a
     place somebody goes to, and a place is an address. */
  const favourites = pathname === "/favourites";
  const scope = parameters.get("in");

  const narrowing = useMemo<Narrowing>(() => {
    const asked = scopeOf(scope);
    return {
      // A grid opened on a library is that library; a search narrowed to one
      // is the same thing said in the address rather than in the path.
      library: id ?? asked.library,
      order,
      descending,
      genre,
      decade,
      search,
      unidentified,
      initial,
      favourites,
      kind: asked.kind,
    };
  }, [id, order, descending, genre, decade, search, unidentified, initial, favourites, scope]);

  /* The cards gathered so far, with the choices they answer. Kept under
     those choices as they grow, so a grid walked back to is drawn again
     whole from its first drawing, as far down as it had been read, rather
     than from its first page. */
  const choices = `grid:${JSON.stringify(narrowing)}`;
  const [gathered, setGathered] = useState<Gathered>(
    () => recall<Gathered>(choices)?.value ?? { choices, cards: [], next: null },
  );
  const [loading, setLoading] = useState(gathered.choices !== choices || gathered.cards.length === 0);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (gathered.choices === choices && (gathered.cards.length > 0 || gathered.next === null)) {
      keep(choices, gathered);
    }
  }, [choices, gathered]);

  /* Any change to the choices starts the grid again from the top, since the
     page after the fiftieth card of one ordering means nothing in another.
     A grid already read under these choices is shown as it was and read
     again quietly, as many cards as it holds, then put in place in one go
     if nobody has read further meanwhile. */
  useEffect(() => {
    const controller = new AbortController();
    const known = recall<Gathered>(choices)?.value;
    setFailed(false);
    if (known) {
      setGathered(known);
      setLoading(false);
      gatherAgain(narrowing, known.cards.length, controller.signal)
        .then((fresh) =>
          setGathered((now) =>
            now.choices === choices && now.cards.length === known.cards.length
              ? { choices, ...fresh }
              : now,
          ),
        )
        .catch(() => {
          // What was shown stays: it was true a moment ago, and the next
          // visit asks again.
        });
      return () => controller.abort();
    }
    setGathered({ choices, cards: [], next: null });
    setLoading(true);
    api
      .works(narrowing, controller.signal)
      .then((page) => {
        setGathered({ choices, cards: page.cards, next: page.next });
        setLoading(false);
      })
      .catch((error) => {
        if (!wasAbandoned(error)) {
          setFailed(true);
          setLoading(false);
        }
      });
    return () => controller.abort();
    // The choices are what the narrowing is written as: one changes with the
    // other.
  }, [choices, narrowing]);

  const filters = useAsked(
    (signal) => (id ? api.filters(id, signal) : Promise.resolve(null)),
    [id],
  );

  const { next } = gathered;
  const loadMore = useCallback(() => {
    if (!next || loading) {
      return;
    }
    setLoading(true);
    api
      .works({ ...narrowing, after: next })
      .then((page) => {
        // Added to rather than replacing: the cards already on screen stay
        // where they are, which is what keeps the scroll position honest.
        setGathered((now) =>
          now.choices === choices
            ? { choices, cards: [...now.cards, ...page.cards], next: page.next }
            : now,
        );
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, [narrowing, choices, next, loading]);

  const choose = useCallback(
    (name: string, value: string | null) => {
      const updated = new URLSearchParams(parameters);
      if (value === null || value === "") {
        updated.delete(name);
      } else {
        updated.set(name, value);
      }
      setParameters(updated, { replace: true });
    },
    [parameters, setParameters],
  );

  return {
    narrowing,
    choose,
    cards: gathered.choices === choices ? gathered.cards : [],
    more: next !== null,
    loadMore,
    loading,
    failed,
    /* A library whose filters could not be read offers none, rather than the
       ones the library before it had. */
    filters: filters.failure ? null : filters.answer,
  };
}
