/*
 * The administration's live line, held once for the page.
 *
 * The server says on it the moment a line of the activity journal is
 * written, and sends what is being watched whenever it changes while
 * something on the page shows it. One line for the whole page, whatever it
 * shows: each one held open is one of the few connections a browser opens to
 * a server. Closed while the page is hidden, and opened again when it comes
 * back, which is also when whatever follows the journal looks again, since
 * nothing was said to it meanwhile.
 *
 * Nobody but an administrator opens it.
 */

import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useAccount } from "./account";
import { api } from "./api";
import type { Watched } from "./api";

/** What the line says about what is being watched: the list, or that it
 *  could not be had. */
export type PlayingNews = { watched: Watched[] } | { cut: true };

interface Line {
  /** Told of every line written in the journal, and whenever the line opens.
   *  Answers how to stop being told. */
  followJournal: (listener: () => void) => () => void;
  /** Told what is being watched whenever it changes, for as long as it
   *  follows. Answers how to stop following. */
  followPlaying: (listener: (news: PlayingNews) => void) => () => void;
}

const LineContext = createContext<Line>({
  followJournal: () => () => {},
  followPlaying: () => () => {},
});

export function AdministrationLine({ children }: { children: ReactNode }) {
  const { account } = useAccount();
  const administrator = account?.is_administrator === true;
  const journalListeners = useRef(new Set<() => void>());
  const playingListeners = useRef(new Set<(news: PlayingNews) => void>());
  /* How many on the page follow what is being watched: the line carries it
     only while at least one does. */
  const [playingFollowers, setPlayingFollowers] = useState(0);
  const wantsPlaying = playingFollowers > 0;

  useEffect(() => {
    if (!administrator) {
      return;
    }
    let line: EventSource | null = null;
    const toPlaying = (news: PlayingNews) => {
      for (const listener of playingListeners.current) listener(news);
    };
    const toJournal = () => {
      for (const listener of journalListeners.current) listener();
    };
    const open = () => {
      line = api.administrationLine(wantsPlaying);
      line.addEventListener("open", toJournal);
      line.addEventListener("activity", toJournal);
      line.addEventListener("playing", (event) =>
        toPlaying({ watched: JSON.parse((event as MessageEvent<string>).data) as Watched[] }),
      );
      line.addEventListener("failed", () => toPlaying({ cut: true }));
      line.onerror = () => toPlaying({ cut: true });
    };
    const close = () => {
      line?.close();
      line = null;
    };
    const followVisibility = () => {
      if (document.visibilityState === "hidden") {
        close();
      } else if (line === null) {
        open();
      }
    };
    if (document.visibilityState !== "hidden") {
      open();
    }
    document.addEventListener("visibilitychange", followVisibility);
    return () => {
      document.removeEventListener("visibilitychange", followVisibility);
      close();
    };
  }, [administrator, wantsPlaying]);

  const followJournal = useCallback((listener: () => void) => {
    journalListeners.current.add(listener);
    return () => {
      journalListeners.current.delete(listener);
    };
  }, []);

  const followPlaying = useCallback((listener: (news: PlayingNews) => void) => {
    playingListeners.current.add(listener);
    setPlayingFollowers((count) => count + 1);
    return () => {
      playingListeners.current.delete(listener);
      setPlayingFollowers((count) => count - 1);
    };
  }, []);

  return <LineContext.Provider value={{ followJournal, followPlaying }}>{children}</LineContext.Provider>;
}

/** Calls `look` every time a line of the journal is written, and when the
 *  line opens again after the page was hidden. */
export function useJournalNews(look: () => void) {
  const { followJournal } = useContext(LineContext);
  const latest = useRef(look);
  latest.current = look;
  useEffect(() => followJournal(() => latest.current()), [followJournal]);
}

/** Follows what is being watched for as long as the caller is on the page. */
export function usePlayingNews(heard: (news: PlayingNews) => void) {
  const { followPlaying } = useContext(LineContext);
  const latest = useRef(heard);
  latest.current = heard;
  useEffect(() => followPlaying((news) => latest.current(news)), [followPlaying]);
}
