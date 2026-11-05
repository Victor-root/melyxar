/*
 * What this device says of itself to the remotes of the account's other
 * devices: how loud it plays, and the queue of its music. The server cannot
 * know either, and a remote showing a volume it made up is worse than one
 * showing none.
 *
 * Said by whatever answers for it on the page: the music when it does, the
 * film on the screen for its own volume while there is one. Said again,
 * gathered, a moment after it last moved, and again whenever the line opens,
 * the server forgetting what a device said once it can no longer be reached.
 */

import { useEffect } from "react";
import { api } from "../api";
import type { Told, ToldQueue } from "../api";
import { useNotificationWords } from "../live";
import type { Queue } from "../music/player/queue";

/** How long what moved is gathered before it is said: a volume dragged is
 *  said once it settles. */
const GATHERED_FOR_MS = 400;

interface Said {
  music: { queue: ToldQueue | null; volume: number; muted: boolean } | null;
  film: { volume: number; muted: boolean } | null;
}

const said: Said = { music: null, film: null };
let waiting: number | null = null;

/** What the device says, the film's volume before the music's while there
 *  is one. */
export function gathered({ music, film }: Said): Told {
  const loudness = film ?? music;
  return {
    volume: loudness?.volume ?? null,
    muted: loudness?.muted ?? null,
    queue: music?.queue ?? null,
  };
}

/** The queue in the order it plays, as a remote shows it. */
export function toldQueue(queue: Queue): ToldQueue | null {
  if (queue.order.length === 0) {
    return null;
  }
  return {
    songs: queue.order.map((place) => queue.songs[place]),
    at: queue.at,
    shuffle: queue.shuffle,
    repeat: queue.repeat,
  };
}

function sayItSoon() {
  if (waiting !== null) {
    window.clearTimeout(waiting);
  }
  waiting = window.setTimeout(() => {
    waiting = null;
    // Refused while the line is closed, which the next opening makes up for.
    void api.tellTheRemotes(gathered(said)).catch(() => {});
  }, GATHERED_FOR_MS);
}

/** Says the music's queue and volume while this page answers for it. */
export function useToldMusic(queue: Queue, volume: number, muted: boolean, answers: boolean) {
  useEffect(() => {
    said.music = answers ? { queue: toldQueue(queue), volume, muted } : null;
    sayItSoon();
  }, [queue, volume, muted, answers]);
  useEffect(
    () => () => {
      said.music = null;
      sayItSoon();
    },
    [],
  );
}

/** Says the volume of the film on the screen, for as long as it is. */
export function useToldFilm(volume: number, muted: boolean) {
  useEffect(() => {
    said.film = { volume, muted };
    sayItSoon();
  }, [volume, muted]);
  useEffect(
    () => () => {
      said.film = null;
      sayItSoon();
    },
    [],
  );
}

/** Says it all again whenever the line opens. Held once, by the page. */
export function useToldAgainOnOpening() {
  useNotificationWords((word) => {
    if (word.name === "open") {
      sayItSoon();
    }
  });
}
