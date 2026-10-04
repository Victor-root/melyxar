/*
 * The page's live line, held once for the page.
 *
 * The server says on it what changes in this account's notifications, the
 * moment it changes, and that the title requests moved. To an administrator it also says the moment a line of
 * the activity journal is written, and sends what is being watched whenever
 * it changes while something on the page shows it. One line for the whole
 * page, whatever it shows: each one held open is one of the few connections
 * a browser opens to a server. Closed while the page is hidden, unless this
 * device asked for the system's notifications, and opened again when it
 * comes back, which is also when whatever follows it looks again, since
 * nothing was said to it meanwhile.
 */

import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useAccount } from "./account";
import { api } from "./api";
import type { Watched } from "./api";

/** What the line says about what is being watched: the list, or that it
 *  could not be had. */
export type PlayingNews = { watched: Watched[] } | { cut: true };

/** The words of the line about this account's notifications. */
const NOTIFICATION_WORDS = [
  "notification",
  "notification_recalled",
  "notifications_read",
  "notifications_unread",
  "notifications_removed",
  "notification_choices",
  "notifications_missed",
] as const;

/** One word about this account's notifications, and what it carries; or
 *  that the line opened again, after which everything is read afresh. */
export type NotificationWord =
  | { name: (typeof NOTIFICATION_WORDS)[number]; data: unknown }
  | { name: "open" };

interface Line {
  /** Told of every line written in the journal, and whenever the line opens.
   *  Answers how to stop being told. */
  followJournal: (listener: () => void) => () => void;
  /** Told what is being watched whenever it changes, for as long as it
   *  follows. Answers how to stop following. */
  followPlaying: (listener: (news: PlayingNews) => void) => () => void;
  /** Told of every word about this account's notifications, for as long as
   *  it follows. */
  followNotifications: (listener: (word: NotificationWord) => void) => () => void;
  /** Told every time the title requests move, and whenever the line opens.
   *  Answers how to stop being told. */
  followRequests: (listener: () => void) => () => void;
  /** Keeps the line open while the page is hidden, for as long as it is
   *  asked: the system's own notifications are said from a hidden page.
   *  Answers how to stop asking. */
  keepWhileHidden: () => () => void;
}

const LineContext = createContext<Line>({
  followJournal: () => () => {},
  followPlaying: () => () => {},
  followNotifications: () => () => {},
  followRequests: () => () => {},
  keepWhileHidden: () => () => {},
});

export function LiveLine({ children }: { children: ReactNode }) {
  const { account } = useAccount();
  const signedIn = account !== null;
  const administrator = account?.is_administrator === true;
  const journalListeners = useRef(new Set<() => void>());
  const playingListeners = useRef(new Set<(news: PlayingNews) => void>());
  const notificationListeners = useRef(new Set<(word: NotificationWord) => void>());
  const requestListeners = useRef(new Set<() => void>());
  /* How many on the page follow what is being watched: the line carries it
     only while at least one does. */
  const [playingFollowers, setPlayingFollowers] = useState(0);
  const wantsPlaying = playingFollowers > 0;
  /* How many on the page need the line while it is hidden. */
  const [hiddenKeepers, setHiddenKeepers] = useState(0);
  const keptWhileHidden = hiddenKeepers > 0;

  useEffect(() => {
    if (!signedIn) {
      return;
    }
    let line: EventSource | null = null;
    const toPlaying = (news: PlayingNews) => {
      for (const listener of playingListeners.current) listener(news);
    };
    const toJournal = () => {
      for (const listener of journalListeners.current) listener();
    };
    const toNotifications = (word: NotificationWord) => {
      for (const listener of notificationListeners.current) listener(word);
    };
    const toRequests = () => {
      for (const listener of requestListeners.current) listener();
    };
    const open = () => {
      line = api.liveLine(administrator && wantsPlaying);
      line.addEventListener("open", () => {
        toNotifications({ name: "open" });
        toRequests();
        if (administrator) toJournal();
      });
      for (const name of NOTIFICATION_WORDS) {
        line.addEventListener(name, (event) =>
          toNotifications({ name, data: JSON.parse((event as MessageEvent<string>).data) as unknown }),
        );
      }
      line.addEventListener("requests", toRequests);
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
        if (!keptWhileHidden) close();
      } else if (line === null) {
        open();
      }
    };
    if (document.visibilityState !== "hidden" || keptWhileHidden) {
      open();
    }
    document.addEventListener("visibilitychange", followVisibility);
    return () => {
      document.removeEventListener("visibilitychange", followVisibility);
      close();
    };
  }, [signedIn, administrator, wantsPlaying, keptWhileHidden]);

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

  const followNotifications = useCallback((listener: (word: NotificationWord) => void) => {
    notificationListeners.current.add(listener);
    return () => {
      notificationListeners.current.delete(listener);
    };
  }, []);

  const followRequests = useCallback((listener: () => void) => {
    requestListeners.current.add(listener);
    return () => {
      requestListeners.current.delete(listener);
    };
  }, []);

  const keepWhileHidden = useCallback(() => {
    setHiddenKeepers((count) => count + 1);
    return () => setHiddenKeepers((count) => count - 1);
  }, []);

  return (
    <LineContext.Provider
      value={{ followJournal, followPlaying, followNotifications, followRequests, keepWhileHidden }}
    >
      {children}
    </LineContext.Provider>
  );
}

/** Calls `look` every time a line of the journal is written, and when the
 *  line opens again after the page was hidden. */
export function useJournalNews(look: () => void) {
  const { followJournal } = useContext(LineContext);
  const latest = useRef(look);
  latest.current = look;
  useEffect(() => followJournal(() => latest.current()), [followJournal]);
}

/** Calls `look` every time the title requests move, and when the line
 *  opens again after the page was hidden. */
export function useRequestsNews(look: () => void) {
  const { followRequests } = useContext(LineContext);
  const latest = useRef(look);
  latest.current = look;
  useEffect(() => followRequests(() => latest.current()), [followRequests]);
}

/** Follows what is being watched for as long as the caller is on the page. */
export function usePlayingNews(heard: (news: PlayingNews) => void) {
  const { followPlaying } = useContext(LineContext);
  const latest = useRef(heard);
  latest.current = heard;
  useEffect(() => followPlaying((news) => latest.current(news)), [followPlaying]);
}

/** Follows every word about this account's notifications for as long as the
 *  caller is on the page. */
export function useNotificationWords(heard: (word: NotificationWord) => void) {
  const { followNotifications } = useContext(LineContext);
  const latest = useRef(heard);
  latest.current = heard;
  useEffect(() => followNotifications((word) => latest.current(word)), [followNotifications]);
}

/** Keeps the line open while the page is hidden, for as long as `keep` holds
 *  and the caller is on the page. */
export function useLineKeptWhileHidden(keep: boolean) {
  const { keepWhileHidden } = useContext(LineContext);
  useEffect(() => (keep ? keepWhileHidden() : undefined), [keep, keepWhileHidden]);
}
