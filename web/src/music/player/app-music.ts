/*
 * The Android application's player of music, when this page runs inside it.
 *
 * The application holds the music (see the decision in
 * docs/architecture/README.md): its player keeps the queue and its rules,
 * plays the sound, counts the listens, tells the server, and goes on with the
 * page gone, for a car among others. The page follows it as a tab follows
 * the one that holds the sound (`tabs.ts`): it is told the queue and where
 * the song has got to, shows them as it always does, and sends every press
 * to the application. Its own elements of sound stay silent.
 *
 * Absent in a browser, where nothing here is used.
 */

import { useCallback, useEffect, useRef } from "react";
import { useAccount } from "../../account";
import type { Song } from "../api";
import { useMusicMarks } from "../marks";
import type { Queue, Repeat } from "./queue";
import type { Beat, Command, Sides } from "./tabs";

/** What the page asks of the application's player. Plain words only. */
export interface AppMusic {
  /** The page's player is in place, signed in as this account: everything
   *  is said to it afresh. */
  hello(account: string): void;
  /** One of the page's commands, with what it carries written as a list. */
  command(name: Command, args: string): void;
  /** The account's choices for its music changed: read them again. */
  preferencesChanged(): void;
}

/** The queue as the application says it, its songs only when they changed
 *  since it last said them. */
interface QueueSaid {
  songs?: Song[];
  order: number[];
  at: number;
  shuffle: boolean;
  repeat: Repeat;
}

declare global {
  interface Window {
    MelyxarMusic?: AppMusic;
    melyxarMusicQueue?: (said: QueueSaid) => void;
    melyxarMusicBeat?: (beat: Beat) => void;
    melyxarMusicListened?: (song: string) => void;
  }
}

/** The application's player, or nothing in a browser. */
export const appMusic: AppMusic | null =
  typeof window !== "undefined" ? (window.MelyxarMusic ?? null) : null;

/** What the page shows before the application has said anything. */
const UNSAID: Beat = { playing: false, waiting: false, position: 0, length: 0, loudness: { volume: 1, muted: false } };

/**
 * The application holding the sound, answered the way `useTabs` answers for
 * the tab that does: this page never holds it, follows what it is told, and
 * sends every command to the application.
 */
export function useApplicationHolds(sides: Sides) {
  const sidesNow = useRef(sides);
  sidesNow.current = sides;
  const followingNow = useRef(true);
  const { account } = useAccount();
  const accountId = account?.id ?? null;
  const { listenCounted } = useMusicMarks();

  useEffect(() => {
    if (!appMusic || !accountId) {
      return;
    }
    let songs: Song[] = [];
    let beat = UNSAID;
    window.melyxarMusicQueue = (said) => {
      songs = said.songs ?? songs;
      const queue: Queue = { songs, order: said.order, at: said.at, shuffle: said.shuffle, repeat: said.repeat };
      sidesNow.current.follow(queue, beat);
    };
    window.melyxarMusicBeat = (said) => {
      beat = said;
      sidesNow.current.follow(null, said);
    };
    window.melyxarMusicListened = () => listenCounted();
    appMusic.hello(accountId);
    return () => {
      window.melyxarMusicQueue = undefined;
      window.melyxarMusicBeat = undefined;
      window.melyxarMusicListened = undefined;
    };
  }, [accountId, listenCounted]);

  const run = useCallback((name: Command, args: unknown[]) => {
    appMusic?.command(name, JSON.stringify(args));
  }, []);

  return { holds: false, followingNow, run };
}
