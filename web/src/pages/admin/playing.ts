/*
 * What is being watched right now, as the administration follows it.
 *
 * Sent by the server the moment anything changes, a film started, paused,
 * moved or left, and every two seconds for the speed of a conversion: nobody
 * reloads anything. The line is closed while the page is hidden, since it
 * holds one of the few connections a browser opens to this server. Shared by
 * the summary and the page of its own, so both count the same way.
 */

import { useEffect, useState } from "react";
import { api } from "../../api";
import type { Watched } from "../../api";

/** What the administration's line last brought, and when it did. */
export interface NowPlaying {
  /** Nothing until the first word. */
  watched: Watched[] | null;
  /** When it came, on this page's clock, for the clocks of the films to run
   *  on from. */
  heardAt: number;
  /** The server could not read the list, or cannot be reached. What was last
   *  shown stays. */
  cut: boolean;
}

export function useNowPlaying(): NowPlaying {
  const [now, setNow] = useState<NowPlaying>({ watched: null, heardAt: 0, cut: false });

  useEffect(() => {
    let line: EventSource | null = null;
    const open = () => {
      line = api.administrationLine();
      line.addEventListener("playing", (event) => {
        const watched = JSON.parse((event as MessageEvent<string>).data) as Watched[];
        setNow({ watched, heardAt: performance.now(), cut: false });
      });
      line.addEventListener("failed", () => setNow((was) => ({ ...was, cut: true })));
      line.onerror = () => setNow((was) => ({ ...was, cut: true }));
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
  }, []);

  return now;
}

/** Where a film has got to at this instant: where the server said it was,
 *  plus the time since when it is playing, never past its end. */
export function positionNow(watched: Watched, heardAt: number, now: number): number {
  if (watched.paused) {
    return watched.position_seconds;
  }
  const reached = watched.position_seconds + Math.max(0, now - heardAt) / 1000;
  return watched.duration_seconds === null ? reached : Math.min(reached, watched.duration_seconds);
}

/** How many films play, how many of them the machine rebuilds, and how many
 *  reach their viewer as they are. */
export interface Counted {
  playing: number;
  rebuilt: number;
  direct: number;
}

/**
 * Counted by what was decided for each. A film only repackaged costs the
 * machine nothing and counts as reaching its viewer as it is; one whose plan
 * was never heard, a player carrying on across a restart, counts as playing
 * and nothing else, since nobody knows which it is.
 */
export function countedOf(watched: Watched[]): Counted {
  return {
    playing: watched.length,
    rebuilt: watched.filter((one) => one.decision?.expensive === true).length,
    direct: watched.filter((one) => one.decision?.expensive === false).length,
  };
}

/** How far through the film its viewer is, from nought to one, when its
 *  length is known. */
export function shareWatched(watched: Watched, position: number): number | null {
  const length = watched.duration_seconds;
  if (length === null || length <= 0) {
    return null;
  }
  return Math.min(1, Math.max(0, position / length));
}

/** Which episode, when it is one: its series and where it sits in it. */
export function episodeOf(
  watched: Watched,
  t: (key: string, values?: Record<string, string | number>) => string,
): string | null {
  if (watched.series === null) {
    return null;
  }
  if (watched.season === null || watched.episode === null) {
    return watched.series;
  }
  return `${watched.series} · ${t("home.up_next.short", {
    season: watched.season,
    episode: watched.episode,
  })}`;
}
