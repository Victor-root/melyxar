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

/** What has been said about one work since the page was drawn. */
interface Mark {
  seen?: Seen;
  favourite?: boolean;
}

interface Marks {
  /** Where this viewer is in a work, their own answer winning. */
  seenOf: (card: Card) => Seen;
  favouriteOf: (card: Card) => boolean;
  /** Says it, everywhere at once, and tells the server. */
  setWatched: (card: Card, watched: boolean) => void;
  setFavourite: (card: Card, favourite: boolean) => void;
}

const MarksContext = createContext<Marks | null>(null);

export function MarksProvider({ children }: { children: ReactNode }) {
  const [said, setSaid] = useState<Record<string, Mark>>({});

  const say = useCallback((id: string, mark: Mark) => {
    setSaid((before) => ({ ...before, [id]: { ...before[id], ...mark } }));
  }, []);

  const setWatched = useCallback(
    (card: Card, watched: boolean) => {
      const before: Seen = said[card.id]?.seen ?? card.seen;
      say(card.id, { seen: watched ? "watched" : "not_started" });
      api.setWatched(card.id, watched).catch(() => say(card.id, { seen: before }));
    },
    [said, say],
  );

  const setFavourite = useCallback(
    (card: Card, favourite: boolean) => {
      const before = said[card.id]?.favourite ?? card.favourite;
      say(card.id, { favourite });
      api.setFavourite(card.id, favourite).catch(() => say(card.id, { favourite: before }));
    },
    [said, say],
  );

  const value = useMemo<Marks>(
    () => ({
      seenOf: (card) => said[card.id]?.seen ?? card.seen,
      favouriteOf: (card) => said[card.id]?.favourite ?? card.favourite,
      setWatched,
      setFavourite,
    }),
    [said, setWatched, setFavourite],
  );

  return <MarksContext.Provider value={value}>{children}</MarksContext.Provider>;
}

export function useMarks(): Marks {
  const marks = useContext(MarksContext);
  if (!marks) {
    throw new Error("a mark was asked for outside its provider");
  }
  return marks;
}
