/*
 * What this account makes of its music, held once for every screen: the
 * songs, albums and artists it likes, and when its listening and its
 * playlists last moved.
 *
 * A heart pressed anywhere changes here first, before the server answers,
 * and goes back if the server refuses, so every heart and every list of
 * favourites showing that song agrees at once. Music's own, apart from the
 * marks of films (`../marks.tsx`), which it shares nothing with.
 *
 * The songs deleted are held here too, on a context of their own: the lists
 * that leave them out do not have to be drawn again each time a heart moves.
 */

import { createContext, useCallback, useContext, useEffect, useMemo, useState, useSyncExternalStore } from "react";
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
  /** Moves each time a playlist is made, changed or deleted, for every
      screen showing one to read it again. */
  playlistsAt: number;
  playlistsHaveMoved: () => void;
  /** Said once songs are deleted, for every list showing them to drop them. */
  setGone: (ids: string[]) => void;
}

const MarksContext = createContext<MusicMarks | null>(null);

/* What is liked, held apart from the rest so that each heart reads only its
   own: a list of a thousand songs is not drawn again because one heart in it
   moved, nor each time a listen is counted. */
const likes = {
  held: new Set<string>() as ReadonlySet<string>,
  listeners: new Set<() => void>(),
  set(next: ReadonlySet<string>) {
    likes.held = next;
    for (const listener of likes.listeners) {
      listener();
    }
  },
  subscribe(listener: () => void) {
    likes.listeners.add(listener);
    return () => likes.listeners.delete(listener);
  },
};

/** Likes or stops liking at once, and goes back if the server refuses. */
function setLiked(id: string, now: boolean) {
  const change = (to: boolean) => {
    const next = new Set(likes.held);
    if (to) {
      next.add(id);
    } else {
      next.delete(id);
    }
    likes.set(next);
  };
  change(now);
  api.setFavourite(id, now).catch(() => change(!now));
}

/** Whether this account likes `id`, and the way to change it. */
export function useLiking(id: string): { liked: boolean; setLiked: (now: boolean) => void } {
  const liked = useSyncExternalStore(likes.subscribe, () => likes.held.has(id));
  return { liked, setLiked: (now: boolean) => setLiked(id, now) };
}
const GoneContext = createContext<ReadonlySet<string>>(new Set());

/** The songs deleted since the page was opened. */
export function useGoneSongs(): ReadonlySet<string> {
  return useContext(GoneContext);
}

export function useMusicMarks(): MusicMarks {
  const marks = useContext(MarksContext);
  if (!marks) {
    throw new Error("the marks of music are not in place");
  }
  return marks;
}

export function MusicMarksProvider({ children }: { children: ReactNode }) {
  const liked = useSyncExternalStore(likes.subscribe, () => likes.held);
  const [listenedAt, setListenedAt] = useState(0);
  const [playlistsAt, setPlaylistsAt] = useState(0);
  const [gone, setGoneHere] = useState<ReadonlySet<string>>(new Set());
  const setGone = useCallback((ids: string[]) => setGoneHere((was) => new Set([...was, ...ids])), []);
  const playlistsHaveMoved = useCallback(() => setPlaylistsAt(Date.now()), []);

  useEffect(() => {
    const stop = new AbortController();
    music
      .favouriteIds(stop.signal)
      .then((ids) => likes.set(new Set(ids)))
      .catch(() => {});
    return () => {
      stop.abort();
      likes.set(new Set());
    };
  }, []);

  const listened = useCallback((song: string) => {
    music
      .recordListen(song)
      .then(() => setListenedAt(Date.now()))
      .catch(() => {});
  }, []);

  const marks = useMemo<MusicMarks>(
    () => ({ liked: (id) => liked.has(id), setLiked, listenedAt, listened, playlistsAt, playlistsHaveMoved, setGone }),
    [liked, listenedAt, listened, playlistsAt, playlistsHaveMoved, setGone],
  );
  return (
    <MarksContext.Provider value={marks}>
      <GoneContext.Provider value={gone}>{children}</GoneContext.Provider>
    </MarksContext.Provider>
  );
}
