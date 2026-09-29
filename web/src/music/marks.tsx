/*
 * What this account makes of its music, held once for every screen: the
 * songs, albums and artists it likes, and when its listening last moved.
 *
 * A heart pressed anywhere changes here first, before the server answers,
 * and goes back if the server refuses, so every heart and every list of
 * favourites showing that song agrees at once. Music's own, apart from the
 * marks of films (`../marks.tsx`), which it shares nothing with.
 */

import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { api } from "../api";
import { music } from "./api";

export interface MusicMarks {
  liked: (id: string) => boolean;
  setLiked: (id: string, liked: boolean) => void;
  /** Moves each time a listen is counted, for the rows of what was
      listened to to be read again. */
  listenedAt: number;
  listened: (song: string) => void;
}

const MarksContext = createContext<MusicMarks | null>(null);

export function useMusicMarks(): MusicMarks {
  const marks = useContext(MarksContext);
  if (!marks) {
    throw new Error("the marks of music are not in place");
  }
  return marks;
}

export function MusicMarksProvider({ children }: { children: ReactNode }) {
  const [liked, setLikedHere] = useState<ReadonlySet<string>>(new Set());
  const [listenedAt, setListenedAt] = useState(0);

  useEffect(() => {
    const stop = new AbortController();
    music
      .favouriteIds(stop.signal)
      .then((ids) => setLikedHere(new Set(ids)))
      .catch(() => {});
    return () => stop.abort();
  }, []);

  const setLiked = useCallback((id: string, now: boolean) => {
    const change = (to: boolean) =>
      setLikedHere((was) => {
        const next = new Set(was);
        if (to) {
          next.add(id);
        } else {
          next.delete(id);
        }
        return next;
      });
    change(now);
    api.setFavourite(id, now).catch(() => change(!now));
  }, []);

  const listened = useCallback((song: string) => {
    music
      .recordListen(song)
      .then(() => setListenedAt(Date.now()))
      .catch(() => {});
  }, []);

  const marks = useMemo<MusicMarks>(
    () => ({ liked: (id) => liked.has(id), setLiked, listenedAt, listened }),
    [liked, setLiked, listenedAt, listened],
  );
  return <MarksContext.Provider value={marks}>{children}</MarksContext.Provider>;
}
