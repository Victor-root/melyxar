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

import { createContext, useCallback, useContext, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { api } from "./api";
import type { Card, Seen } from "./api";
import { markedWatched, whereaboutsOf } from "./watching";
import type { Said, Whereabouts } from "./watching";

/** What has been said about one work since the page was drawn. */
interface Mark {
  watched?: Said;
  favourite?: boolean;
  /** On the server's own front shelf. Unlike the two above, nothing is known
      about this until somebody says it here: a card does not arrive saying
      whether it is on the front page. */
  pinned?: boolean;
  /** Deleted from here. Nothing draws it again: a card that stays in a grid
      after its work has gone is a card that fails when it is pressed. */
  gone?: boolean;
}

interface Marks {
  /** Where this viewer is in a work, their own answer winning. */
  seenOf: (card: Card) => Seen;
  /** Where to carry on from, in seconds, and nothing when there is nowhere. */
  resumeOf: (card: Card) => number | null;
  favouriteOf: (card: Card) => boolean;
  /** Whether this work was put on the front page from this page, and nothing
      at all when nobody has said. */
  pinnedOf: (card: Card) => boolean | undefined;
  /** Whether this work was deleted from this page. */
  goneOf: (id: string) => boolean;
  /** Says works were deleted, everywhere at once. The server has already
      done it: this is what was answered, not what was hoped for. */
  setGone: (ids: string[]) => void;
  /** Says it, everywhere at once, and tells the server. */
  setWatched: (card: Card, watched: boolean) => void;
  setFavourite: (card: Card, favourite: boolean) => void;
  setPinned: (card: Card, pinned: boolean) => void;
  /** Bumped whenever something said here changes which works a row built by
      the server holds: a work put on the front page, an episode ticked off
      the row of what is unfinished. The screens standing on such a row read
      it again rather than mending it here, since what replaces what left is
      the server's answer and nobody else's. */
  rowsMoved: number;
  /** Says that something outside this store changed what a row of the server's
      holds: a work named by hand is a different card with a different title
      and a different poster. */
  rowsHaveMoved: () => void;
}

const MarksContext = createContext<Marks | null>(null);

export function MarksProvider({ children }: { children: ReactNode }) {
  const [said, setSaid] = useState<Record<string, Mark>>({});
  const [rowsMoved, setRowsMoved] = useState(0);
  const rowsHaveMoved = useCallback(() => setRowsMoved((count) => count + 1), []);

  const say = useCallback((id: string, mark: Mark) => {
    setSaid((before) => ({ ...before, [id]: { ...before[id], ...mark } }));
  }, []);

  const setWatched = useCallback(
    (card: Card, watched: boolean) => {
      const before = said[card.id]?.watched;
      const sent = sentOf(card);
      say(card.id, {
        watched: { said: markedWatched(watched, whereaboutsOf(sent, before)), over: sent },
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
    [said, say, rowsHaveMoved],
  );

  const setFavourite = useCallback(
    (card: Card, favourite: boolean) => {
      const before = said[card.id]?.favourite ?? card.favourite;
      say(card.id, { favourite });
      api.setFavourite(card.id, favourite).catch(() => say(card.id, { favourite: before }));
    },
    [said, say],
  );

  /* The banner of the home page is the one thing this changes, and it is the
     server that decides what stands in it. So nothing is mended here beyond
     the menu entry's own wording: what is said is that the shelf moved, and
     the screens that show it ask again. */
  const setPinned = useCallback(
    (card: Card, pinned: boolean) => {
      const before = said[card.id]?.pinned;
      say(card.id, { pinned });
      rowsHaveMoved();
      api.setPinned(card.id, pinned).catch(() => say(card.id, { pinned: before }));
    },
    [said, say, rowsHaveMoved],
  );

  /* What stood around a deleted work is the server's to redraw: a series
     that lost its last episode, a folder that lost a photo. */
  const setGone = useCallback(
    (ids: string[]) => {
      setSaid((before) => {
        const after = { ...before };
        for (const id of ids) {
          after[id] = { ...before[id], gone: true };
        }
        return after;
      });
      rowsHaveMoved();
    },
    [rowsHaveMoved],
  );

  const value = useMemo<Marks>(
    () => ({
      seenOf: (card) => whereaboutsOf(sentOf(card), said[card.id]?.watched).seen,
      resumeOf: (card) => whereaboutsOf(sentOf(card), said[card.id]?.watched).resume,
      favouriteOf: (card) => said[card.id]?.favourite ?? card.favourite,
      pinnedOf: (card) => said[card.id]?.pinned,
      goneOf: (id) => said[id]?.gone === true,
      setWatched,
      setFavourite,
      setPinned,
      setGone,
      rowsMoved,
      rowsHaveMoved,
    }),
    [said, setWatched, setFavourite, setPinned, setGone, rowsMoved, rowsHaveMoved],
  );

  return <MarksContext.Provider value={value}>{children}</MarksContext.Provider>;
}

/** Where the server said this viewer is in a work. */
function sentOf(card: Card): Whereabouts {
  return { seen: card.seen, resume: card.resume_from_seconds };
}

export function useMarks(): Marks {
  const marks = useContext(MarksContext);
  if (!marks) {
    throw new Error("a mark was asked for outside its provider");
  }
  return marks;
}
