/*
 * What is being watched right now, as the administration follows it.
 *
 * Read on a short beat: a film paused, started or left on another screen is
 * seen here within two seconds, without anybody reloading anything. Shared by
 * the summary and the page of its own, so both count the same way.
 */

import { useEffect } from "react";
import { api } from "../../api";
import type { Watched } from "../../api";
import type { Asked } from "../../asking";
import { useAsked } from "../../asking";

/** How often what is being watched is read again. */
const LOOKED_AT_EVERY_MS = 2_000;

export function useNowPlaying(): Asked<Watched[]> {
  const playing = useAsked((signal) => api.nowPlaying(signal));
  const { look } = playing;
  useEffect(() => {
    const timer = window.setInterval(look, LOOKED_AT_EVERY_MS);
    return () => window.clearInterval(timer);
  }, [look]);
  return playing;
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
export function shareWatched(watched: Watched): number | null {
  const length = watched.duration_seconds;
  if (length === null || length <= 0) {
    return null;
  }
  return Math.min(1, Math.max(0, watched.position_seconds / length));
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
