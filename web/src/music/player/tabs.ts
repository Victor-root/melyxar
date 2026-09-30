/*
 * One player of music for the whole browser. Whatever the number of tabs,
 * one of them holds the sound and the others show it and steer it: a song
 * started, paused or skipped in any tab is heard from that one, and every
 * bar shows the same queue and the same place in the song.
 *
 * The tab that holds the sound says what it plays at every change and once
 * a second. A tab that holds none takes that as what it shows, moves the
 * clock on by itself between two words, and sends its commands to the one
 * that holds it. With no tab holding the sound, the tab that is asked for
 * something takes it, where the song was left.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import type { Loudness } from "./kept";
import type { Queue } from "./queue";

const CHANNEL = "melyxar.music.tabs";
/** How often the tab holding the sound says where the song has got to. */
const BEAT_MS = 1_000;
/** Without a word for this long, the tab holding the sound is gone. */
const STALE_MS = 3_500;
/** How often a tab that holds no sound moves the clock on. */
const TICK_MS = 250;

export type Command =
  | "play"
  | "toggle"
  | "pause"
  | "resume"
  | "stop"
  | "next"
  | "previous"
  | "seek"
  | "setVolume"
  | "setMuted"
  | "toggleShuffle"
  | "cycleRepeat"
  | "jump"
  | "playNext"
  | "playLast"
  | "remove";

/** What changes as the music plays, and what every bar shows of it. */
export interface Beat {
  playing: boolean;
  waiting: boolean;
  position: number;
  length: number;
  loudness: Loudness;
}

type Body =
  | { kind: "hello" }
  | { kind: "claim" }
  | { kind: "state"; queue: Queue; beat: Beat }
  | { kind: "beat"; beat: Beat }
  | { kind: "gone"; beat: Beat }
  | { kind: "command"; to: string; name: Command; args: unknown[] };

type Message = Body & { from: string };

/** Where the song has got to `elapsedMs` after a word that said so. */
export function positionAfter(beat: Beat, elapsedMs: number): number {
  if (!beat.playing) {
    return beat.position;
  }
  const moved = Math.floor(beat.position + elapsedMs / 1000);
  return beat.length > 0 ? Math.min(moved, beat.length) : moved;
}

/** Whether a tab that spoke at `heardAt` is still there to hold the sound. */
export function isThere(heardAt: number, now: number): boolean {
  return now - heardAt < STALE_MS;
}

/** Of two tabs that took the sound at once, the one that keeps it. */
export function keepsTheSound(mine: string, theirs: string): boolean {
  return mine < theirs;
}

export interface Sides {
  queue: () => Queue;
  beat: () => Beat;
  /** What the tab holding the sound plays, shown here: the queue when it
      has changed, and where the song has got to. */
  follow: (queue: Queue | null, beat: Beat) => void;
  /** Another tab took the sound: it is let go of here, the queue kept. */
  letGo: () => void;
  /** A command another tab sent, done here. */
  obey: (name: Command, args: unknown[]) => void;
}

interface Holder {
  id: string;
  heardAt: number;
  beat: Beat;
}

export function useTabs(
  sides: Sides,
  watched: { queue: Queue; playing: boolean; waiting: boolean; loudness: Loudness },
) {
  const sidesNow = useRef(sides);
  sidesNow.current = sides;
  const [id] = useState(() => Math.random().toString(36).slice(2));
  const [holds, setHolds] = useState(false);
  const holdsNow = useRef(false);
  /* Whether another tab holds the sound, or did: what this tab shows is then
     theirs, and its own elements of sound stay silent. */
  const followingNow = useRef(false);
  const channel = useRef<BroadcastChannel | null>(null);
  const holder = useRef<Holder | null>(null);

  const post = useCallback((body: Body) => channel.current?.postMessage({ ...body, from: id } satisfies Message), [id]);
  const postState = useCallback(
    () => post({ kind: "state", queue: sidesNow.current.queue(), beat: sidesNow.current.beat() }),
    [post],
  );
  const postBeat = useCallback(() => post({ kind: "beat", beat: sidesNow.current.beat() }), [post]);
  const hold = useCallback((value: boolean) => {
    holdsNow.current = value;
    setHolds(value);
  }, []);

  useEffect(() => {
    if (typeof BroadcastChannel === "undefined") {
      return;
    }
    const opened = new BroadcastChannel(CHANNEL);
    channel.current = opened;
    const heardFrom = (from: string, beat: Beat) => {
      holder.current = { id: from, heardAt: Date.now(), beat };
      followingNow.current = true;
    };
    opened.onmessage = (event: MessageEvent<Message>) => {
      const message = event.data;
      if (message.from === id) {
        return;
      }
      switch (message.kind) {
        case "hello":
          if (holdsNow.current) {
            postState();
          }
          break;
        case "claim":
          if (holdsNow.current) {
            if (keepsTheSound(id, message.from)) {
              break;
            }
            hold(false);
            sidesNow.current.letGo();
          }
          heardFrom(message.from, sidesNow.current.beat());
          break;
        case "state":
          if (!holdsNow.current) {
            heardFrom(message.from, message.beat);
            sidesNow.current.follow(message.queue, message.beat);
          }
          break;
        case "beat":
          if (holdsNow.current) {
            break;
          }
          if (holder.current?.id === message.from) {
            heardFrom(message.from, message.beat);
            sidesNow.current.follow(null, message.beat);
          } else {
            post({ kind: "hello" });
          }
          break;
        case "gone":
          if (holder.current?.id === message.from) {
            holder.current = null;
            sidesNow.current.follow(null, { ...message.beat, playing: false });
          }
          break;
        case "command":
          if (holdsNow.current && message.to === id) {
            sidesNow.current.obey(message.name, message.args);
          }
          break;
      }
    };
    post({ kind: "hello" });
    return () => {
      opened.close();
      channel.current = null;
    };
  }, [id, post, postState, hold]);

  // Holding the sound: every change of the queue is said, and the tab lets
  // go of the sound for good once there is nothing left to play.
  useEffect(() => {
    if (!holds) {
      return;
    }
    postState();
    if (watched.queue.songs.length === 0) {
      post({ kind: "gone", beat: sidesNow.current.beat() });
      hold(false);
    }
  }, [holds, watched.queue, post, postState, hold]);

  useEffect(() => {
    if (holds) {
      postBeat();
    }
  }, [holds, watched.playing, watched.waiting, watched.loudness, postBeat]);

  useEffect(() => {
    if (!holds) {
      return;
    }
    const every = window.setInterval(postBeat, BEAT_MS);
    const away = () => post({ kind: "gone", beat: { ...sidesNow.current.beat(), playing: false } });
    window.addEventListener("pagehide", away);
    return () => {
      window.clearInterval(every);
      window.removeEventListener("pagehide", away);
    };
  }, [holds, post, postBeat]);

  // Not holding it: the clock moves on between two words, and stops when
  // the tab holding the sound falls silent.
  useEffect(() => {
    if (holds) {
      return;
    }
    const every = window.setInterval(() => {
      const there = holder.current;
      if (!there || !there.beat.playing) {
        return;
      }
      const now = Date.now();
      if (isThere(there.heardAt, now)) {
        sidesNow.current.follow(null, { ...there.beat, position: positionAfter(there.beat, now - there.heardAt) });
      } else {
        there.beat = { ...there.beat, playing: false };
        sidesNow.current.follow(null, there.beat);
      }
    }, TICK_MS);
    return () => window.clearInterval(every);
  }, [holds]);

  /* Done here when this tab holds the sound, sent to the tab that does
     when there is one, and otherwise done here after taking the sound. */
  const run = useCallback(
    (name: Command, args: unknown[], local: () => void) => {
      if (holdsNow.current) {
        local();
        return;
      }
      const there = holder.current;
      if (there && isThere(there.heardAt, Date.now())) {
        post({ kind: "command", to: there.id, name, args });
        return;
      }
      if (name === "pause" || name === "resume") {
        return;
      }
      followingNow.current = false;
      holder.current = null;
      hold(true);
      post({ kind: "claim" });
      local();
    },
    [post, hold],
  );

  return { holds, followingNow, run };
}
