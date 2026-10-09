/*
 * What this viewer has made of a work, held in one place for every screen.
 *
 * Pressing the tick on a card has to change that card, the same film further
 * down the same page, the row it also stands in, and the page somebody walks
 * back to afterwards. A card that kept its own answer would leave four
 * screens disagreeing about one film until the next reload, which is exactly
 * what the working rules of this project forbid.
 *
 * So a mark is not kept by the card that was pressed. It is kept here, by
 * work, and every card reads its own mark through this before drawing itself.
 * A card that nobody has touched since the server sent it reads what the
 * server sent.
 *
 * Written before the server has answered and put back if it refuses: the
 * press is what somebody did, and a tick that waits for a round trip before
 * moving is a tick people press twice.
 */

import { createContext, useCallback, useContext, useMemo, useState, useSyncExternalStore } from "react";
import type { ReactNode } from "react";
import { api } from "./api";
import type { Card, Seen } from "./api";
import { createMarkStore } from "./marks-store";
import type { Mark, MarkStore } from "./marks-store";
import { markedWatched, whereaboutsOf } from "./watching";
import type { Whereabouts } from "./watching";

interface Marks extends MarkActions {
  /** Where this viewer is in a work, their own answer winning. */
  seenOf: (card: Card) => Seen;
  /** Where to carry on from, in seconds, and nothing when there is nowhere. */
  resumeOf: (card: Card) => number | null;
  /** How many episodes of a series or a season are left. */
  unwatchedOf: (card: Card) => number;
  favouriteOf: (card: Card) => boolean;
  watchLaterOf: (card: Card) => boolean;
  /** Whether this work was put on the front page from this page, and nothing
      at all when nobody has said. */
  pinnedOf: (card: Card) => boolean | undefined;
  /** Whether this work was deleted from this page. */
  goneOf: (id: string) => boolean;
  /** Bumped whenever something said here changes which works a row built by
      the server holds: a work put on the front page, an episode ticked off
      the row of what is unfinished. The screens standing on such a row read
      it again rather than mending it here, since what replaces what left is
      the server's answer and nobody else's. */
  rowsMoved: number;
}

/** What can be said about a work. The same from the first drawing to the
 *  last, so that whoever only says something is never drawn again for it. */
interface MarkActions {
  /** Says works were deleted, everywhere at once. The server has already
      done it: this is what was answered, not what was hoped for. */
  setGone: (ids: string[]) => void;
  /** Says it, everywhere at once, and tells the server. */
  setWatched: (card: Card, watched: boolean) => void;
  setFavourite: (card: Card, favourite: boolean) => void;
  setWatchLater: (card: Card, later: boolean) => void;
  /** Settled once the server holds it, for pins that have to land in turn. */
  setPinned: (card: Card, pinned: boolean) => Promise<void>;
  /** Says that something outside this store changed what a row of the server's
      holds: a work named by hand is a different card with a different title
      and a different poster. */
  rowsHaveMoved: () => void;
}

const MarksContext = createContext<Marks | null>(null);
const MarkActionsContext = createContext<MarkActions | null>(null);
const MarkStoreContext = createContext<MarkStore | null>(null);

export function MarksProvider({ children }: { children: ReactNode }) {
  const [store] = useState(createMarkStore);
  const said = useSyncExternalStore(store.subscribe, store.all);
  const [rowsMoved, setRowsMoved] = useState(0);
  const rowsHaveMoved = useCallback(() => setRowsMoved((count) => count + 1), []);

  const say = useCallback((id: string, mark: Mark) => store.say({ [id]: mark }), [store]);

  const setWatched = useCallback(
    (card: Card, watched: boolean) => {
      const before = store.of(card.id)?.watched;
      const sent = sentOf(card);
      say(card.id, {
        watched: {
          said: markedWatched(watched, whereaboutsOf(sent, before), card.episodes),
          over: sent,
        },
        /* Watched is what later was waiting for: the server takes it off
           the list too. */
        ...(watched && { watchLater: { said: false, over: card.watch_later } }),
      });
      /* A work ticked off is a work that has left the row of what is
         unfinished, and the one the series above it is waiting on is not
         the same episode any more. Said once the server holds it: read
         again before, a screen gets the answer from before the mark. */
      api
        .setWatched(card.id, watched)
        .then(rowsHaveMoved)
        .catch(() => say(card.id, { watched: before }));
    },
    [store, say, rowsHaveMoved],
  );

  const setFavourite = useCallback(
    (card: Card, favourite: boolean) => {
      const before = store.of(card.id)?.favourite ?? card.favourite;
      say(card.id, { favourite });
      api.setFavourite(card.id, favourite).catch(() => say(card.id, { favourite: before }));
    },
    [store, say],
  );

  const setWatchLater = useCallback(
    (card: Card, later: boolean) => {
      const before = store.of(card.id)?.watchLater;
      say(card.id, { watchLater: { said: later, over: card.watch_later } });
      api.setWatchLater(card.id, later).catch(() => say(card.id, { watchLater: before }));
    },
    [store, say],
  );

  /* The banner of the home page is the one thing this changes, and it is the
     server that decides what stands in it. So nothing is mended here beyond
     the menu entry's own wording: what is said is that the shelf moved, and
     the screens that show it ask again. What made room for it in a full
     banner is no longer there either. */
  const setPinned = useCallback(
    (card: Card, pinned: boolean) => {
      const before = store.of(card.id)?.pinned;
      say(card.id, { pinned });
      rowsHaveMoved();
      return api.setPinned(card.id, pinned).then(
        ({ displaced }) => {
          displaced.forEach((id) => say(id, { pinned: false }));
          if (displaced.length > 0) {
            rowsHaveMoved();
          }
        },
        () => say(card.id, { pinned: before }),
      );
    },
    [store, say, rowsHaveMoved],
  );

  /* What stood around a deleted work is the server's to redraw: a series
     that lost its last episode, a folder that lost a photo. */
  const setGone = useCallback(
    (ids: string[]) => {
      store.say(Object.fromEntries(ids.map((id) => [id, { gone: true }])));
      rowsHaveMoved();
    },
    [store, rowsHaveMoved],
  );

  const actions = useMemo<MarkActions>(
    () => ({ setWatched, setFavourite, setWatchLater, setPinned, setGone, rowsHaveMoved }),
    [setWatched, setFavourite, setWatchLater, setPinned, setGone, rowsHaveMoved],
  );

  const value = useMemo<Marks>(
    () => ({
      ...actions,
      seenOf: (card) => whereaboutsOf(sentOf(card), said[card.id]?.watched).seen,
      resumeOf: (card) => whereaboutsOf(sentOf(card), said[card.id]?.watched).resume,
      unwatchedOf: (card) => whereaboutsOf(sentOf(card), said[card.id]?.watched).unwatched,
      favouriteOf: (card) => said[card.id]?.favourite ?? card.favourite,
      watchLaterOf: (card) => {
        const later = said[card.id]?.watchLater;
        return later && later.over === card.watch_later ? later.said : card.watch_later;
      },
      pinnedOf: (card) => said[card.id]?.pinned,
      goneOf: (id) => said[id]?.gone === true,
      rowsMoved,
    }),
    [actions, said, rowsMoved],
  );

  return (
    <MarkStoreContext.Provider value={store}>
      <MarkActionsContext.Provider value={actions}>
        <MarksContext.Provider value={value}>{children}</MarksContext.Provider>
      </MarkActionsContext.Provider>
    </MarkStoreContext.Provider>
  );
}

/** Where the server said this viewer is in a work. */
function sentOf(card: Card): Whereabouts {
  return { seen: card.seen, resume: card.resume_from_seconds, unwatched: card.unwatched };
}

/** Everything said about every work, and what can be said. Drawn again at
 *  every change of any of them: for a screen that reads across works. */
export function useMarks(): Marks {
  const marks = useContext(MarksContext);
  if (!marks) {
    throw new Error("a mark was asked for outside its provider");
  }
  return marks;
}

/** What can be said about a work, without being drawn again when something
 *  is: for whatever only says. */
export function useMarkActions(): MarkActions {
  const actions = useContext(MarkActionsContext);
  if (!actions) {
    throw new Error("a mark was asked for outside its provider");
  }
  return actions;
}

/** Where this viewer is in one work, and what they said about it. */
export interface MarksOf extends Whereabouts {
  favourite: boolean;
  gone: boolean;
}

/** What was said about one work, and nothing about any other: the card is
 *  drawn again when its own work changes and not when another one does. */
export function useMarksOf(card: Card): MarksOf {
  const store = useContext(MarkStoreContext);
  if (!store) {
    throw new Error("a mark was asked for outside its provider");
  }
  const said = useSyncExternalStore(store.subscribe, () => store.of(card.id));
  return {
    ...whereaboutsOf(sentOf(card), said?.watched),
    favourite: said?.favourite ?? card.favourite,
    gone: said?.gone === true,
  };
}
