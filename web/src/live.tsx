/*
 * The page's live line, held once for the page.
 *
 * The server says on it what changes in this account's notifications, the
 * moment it changes, that the title requests or what the libraries hold
 * moved, and where maintenance stands as it moves. To an administrator it
 * also says the moment a line of the activity journal is written, and sends
 * what is being watched whenever it changes while something on the page
 * shows it. It also carries what another device of the account asks of
 * this one, and, while a remote on the page steers them, which of the
 * account's other devices can be reached and what plays there. One line for
 * the whole page, whatever it shows: each one held open is one of the few
 * connections a browser opens to a server. Closed
 * while the page is hidden, unless this device asked for the system's
 * notifications, and opened again when it comes back, which is also when
 * whatever follows it looks again, since nothing was said to it meanwhile.
 */

import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useAccount } from "./account";
import { serverMoved } from "./player/logo";
import { api } from "./api";
import type { Maintenance, RemoteOrder, SteerableDevice, Watched } from "./api";
import { askAboutMaintenance, maintenanceHeard } from "./maintenance";

/** What the line says about what is being watched: the list, or that it
 *  could not be had. */
export type PlayingNews = { watched: Watched[] } | { cut: true };

/** What the line says about the account's other devices: the list and when
 *  it was told, or that it could not be had. */
export type DevicesNews = { devices: SteerableDevice[]; heardAt: number } | { cut: true };

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
  /** Told every time what a library holds changes, and whenever the line
   *  opens. Answers how to stop being told. */
  followLibraries: (listener: () => void) => () => void;
  /** Told every order another device of the account gives this one, for as
   *  long as it follows. */
  followOrders: (listener: (order: RemoteOrder) => void) => () => void;
  /** Told the account's other devices whenever they change, for as long as
   *  it follows. */
  followDevices: (listener: (news: DevicesNews) => void) => () => void;
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
  followLibraries: () => () => {},
  followOrders: () => () => {},
  followDevices: () => () => {},
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
  const libraryListeners = useRef(new Set<() => void>());
  const orderListeners = useRef(new Set<(order: RemoteOrder) => void>());
  const deviceListeners = useRef(new Set<(news: DevicesNews) => void>());
  /* The devices as last told, handed at once to whatever starts following
     them while others already do: the line says them again only when they
     change. */
  const lastDevices = useRef<DevicesNews | null>(null);
  /* How many on the page steer the other devices: the line carries them only
     while at least one does. */
  const [deviceFollowers, setDeviceFollowers] = useState(0);
  const wantsDevices = deviceFollowers > 0;
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
    const toLibraries = () => {
      for (const listener of libraryListeners.current) listener();
    };
    const toOrders = (order: RemoteOrder) => {
      for (const listener of orderListeners.current) listener(order);
    };
    const toDevices = (news: DevicesNews) => {
      lastDevices.current = news;
      for (const listener of deviceListeners.current) listener(news);
    };
    const open = () => {
      line = api.liveLine(administrator && wantsPlaying, wantsDevices);
      line.addEventListener("open", () => {
        toNotifications({ name: "open" });
        toRequests();
        toLibraries();
        askAboutMaintenance();
        if (administrator) toJournal();
      });
      for (const name of NOTIFICATION_WORDS) {
        line.addEventListener(name, (event) =>
          toNotifications({ name, data: JSON.parse((event as MessageEvent<string>).data) as unknown }),
        );
      }
      line.addEventListener("requests", toRequests);
      /* What the server wears for everybody moved: its name, logo, theme or
         accent. An account's own choices are read where it changes them. */
      line.addEventListener("looks", (event) => {
        if ((event as MessageEvent<string>).data === "server") {
          serverMoved();
        }
      });
      line.addEventListener("libraries", toLibraries);
      line.addEventListener("activity", toJournal);
      line.addEventListener("maintenance", (event) =>
        maintenanceHeard(JSON.parse((event as MessageEvent<string>).data) as Maintenance),
      );
      line.addEventListener("playing", (event) =>
        toPlaying({ watched: JSON.parse((event as MessageEvent<string>).data) as Watched[] }),
      );
      line.addEventListener("failed", () => toPlaying({ cut: true }));
      line.addEventListener("remote", (event) =>
        toOrders(JSON.parse((event as MessageEvent<string>).data) as RemoteOrder),
      );
      line.addEventListener("devices", (event) =>
        toDevices({ devices: JSON.parse((event as MessageEvent<string>).data) as SteerableDevice[], heardAt: Date.now() }),
      );
      line.addEventListener("devices_failed", () => toDevices({ cut: true }));
      line.onerror = () => {
        toPlaying({ cut: true });
        toDevices({ cut: true });
      };
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
  }, [signedIn, administrator, wantsPlaying, wantsDevices, keptWhileHidden]);

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

  const followLibraries = useCallback((listener: () => void) => {
    libraryListeners.current.add(listener);
    return () => {
      libraryListeners.current.delete(listener);
    };
  }, []);

  const followOrders = useCallback((listener: (order: RemoteOrder) => void) => {
    orderListeners.current.add(listener);
    return () => {
      orderListeners.current.delete(listener);
    };
  }, []);

  const followDevices = useCallback((listener: (news: DevicesNews) => void) => {
    deviceListeners.current.add(listener);
    if (lastDevices.current !== null) {
      listener(lastDevices.current);
    }
    setDeviceFollowers((count) => count + 1);
    return () => {
      deviceListeners.current.delete(listener);
      if (deviceListeners.current.size === 0) {
        lastDevices.current = null;
      }
      setDeviceFollowers((count) => count - 1);
    };
  }, []);

  const keepWhileHidden = useCallback(() => {
    setHiddenKeepers((count) => count + 1);
    return () => setHiddenKeepers((count) => count - 1);
  }, []);

  const line = useMemo(
    () => ({
      followJournal,
      followPlaying,
      followNotifications,
      followRequests,
      followLibraries,
      followOrders,
      followDevices,
      keepWhileHidden,
    }),
    [
      followJournal,
      followPlaying,
      followNotifications,
      followRequests,
      followLibraries,
      followOrders,
      followDevices,
      keepWhileHidden,
    ],
  );

  return <LineContext.Provider value={line}>{children}</LineContext.Provider>;
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

/** Calls `look` every time what a library holds changes, and when the line
 *  opens again after the page was hidden. */
export function useLibrariesNews(look: () => void) {
  const { followLibraries } = useContext(LineContext);
  const latest = useRef(look);
  latest.current = look;
  useEffect(() => followLibraries(() => latest.current()), [followLibraries]);
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

/** Follows the orders another device of the account gives this one, for as
 *  long as `obey` is given and the caller is on the page. */
export function useRemoteOrders(obey: ((order: RemoteOrder) => void) | null) {
  const { followOrders } = useContext(LineContext);
  const latest = useRef(obey);
  latest.current = obey;
  const listening = obey !== null;
  useEffect(
    () => (listening ? followOrders((order) => latest.current?.(order)) : undefined),
    [listening, followOrders],
  );
}

/** Follows the account's other devices for as long as the caller is on the
 *  page. */
export function useDevicesNews(heard: (news: DevicesNews) => void) {
  const { followDevices } = useContext(LineContext);
  const latest = useRef(heard);
  latest.current = heard;
  useEffect(() => followDevices((news) => latest.current(news)), [followDevices]);
}
