/*
 * The player of music: one element of sound for the whole interface, made
 * once and never unmounted, so that going from page to page never stops a
 * song. See `docs/architecture/06-musique.md`.
 *
 * Two things are handed out apart. What is playing and every command change
 * rarely, and every page that has a play button reads them. Where the song
 * has got to changes four times a second, and only the bar and the page of
 * what is playing read it: a list of songs redrawn four times a second while
 * music plays is a list that stutters when scrolled.
 */

import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import type { ReactNode } from "react";
import { useIsAFilmOnScreen } from "../../on-screen";
import { music as server } from "../api";
import type { MusicPreferences, Song } from "../api";
import { formsPlayedHere } from "./forms";
import { rememberLoudness, rememberQueue, storedLoudness, storedQueue } from "./kept";
import type { Loudness } from "./kept";
import {
  EMPTY,
  afterTheEnd,
  backward,
  current,
  forward,
  jumpTo,
  nextRepeat,
  playLast,
  playNext,
  playing as playingFrom,
  withShuffle,
  without,
} from "./queue";
import type { Queue } from "./queue";
import { MediaSessionPosition, useMediaSession } from "./session";

export interface Music {
  queue: Queue;
  /** The song playing or paused, if there is one. */
  song: Song | null;
  playing: boolean;
  /** Waiting for the sound to arrive. */
  waiting: boolean;
  loudness: Loudness;
  /** Whether the page of what is playing is open over the interface. */
  open: boolean;
  /** Plays these songs from the one at `index`, replacing the queue. */
  play: (songs: Song[], index: number, shuffle?: boolean) => void;
  toggle: () => void;
  /** Stops for good: the queue goes, and the bar with it. */
  stop: () => void;
  next: () => void;
  previous: () => void;
  seek: (seconds: number) => void;
  setVolume: (volume: number) => void;
  setMuted: (muted: boolean) => void;
  toggleShuffle: () => void;
  cycleRepeat: () => void;
  jump: (at: number) => void;
  playNext: (songs: Song[]) => void;
  playLast: (songs: Song[]) => void;
  remove: (at: number) => void;
  setOpen: (open: boolean) => void;
  /** What this account chose for its music, the defaults until the server
      has said. */
  preferences: MusicPreferences;
  /** Changed here at once, and put back as it was if the server refuses. */
  setPreferences: (chosen: MusicPreferences) => Promise<void>;
}

/** Where the song has got to, and how long it runs, in seconds. */
export interface Time {
  position: number;
  length: number;
}

const MusicContext = createContext<Music | null>(null);

/** Where the song has got to, read only by what shows it. */
const time = {
  now: { position: 0, length: 0 } as Time,
  listeners: new Set<() => void>(),
  set(next: Time) {
    if (next.position !== this.now.position || next.length !== this.now.length) {
      this.now = next;
      for (const listener of this.listeners) {
        listener();
      }
    }
  },
};

export function useMusic(): Music {
  const music = useContext(MusicContext);
  if (!music) {
    throw new Error("the player of music is not in place");
  }
  return music;
}

export function useMusicTime(): Time {
  return useSyncExternalStore(
    (listener) => {
      time.listeners.add(listener);
      return () => time.listeners.delete(listener);
    },
    () => time.now,
  );
}

/** What an account that chose nothing gets, as the server has it. */
export const DEFAULT_PREFERENCES: MusicPreferences = {
  film_on_screen: "stop",
  resume_queue: true,
  max_bitrate_kbps: null,
};

/** How often where the song has got to is written down, for a tab closed
 *  and opened again to take it up there. */
const KEEP_EVERY_MS = 5_000;

export function MusicProvider({ children }: { children: ReactNode }) {
  const [audio] = useState(() => {
    const element = new Audio();
    element.preload = "metadata";
    return element;
  });
  const kept = useMemo(storedQueue, []);
  /* Empty until the account has said whether a queue left in a closed tab
     is taken up again: shown and then taken away, it would be a bar that
     flickers on every visit. */
  const [queue, setQueue] = useState<Queue>(EMPTY);
  const [preferences, setPreferencesHere] = useState<MusicPreferences | null>(null);
  const [playing, setPlaying] = useState(false);
  const [waiting, setWaiting] = useState(false);
  const [loudness, setLoudness] = useState<Loudness>(storedLoudness);
  const [open, setOpen] = useState(false);
  const song = current(queue);

  /* A song converted on its way cannot be moved about in: a move is asked
     of the server from where it lands, and the element counts from there. */
  const offset = useRef(0);
  const startAt = useRef(0);
  /* Where the queue taken up from an earlier visit had got to, used once. */
  const resumeFrom = useRef(0);
  /* Whether the next song asked for plays at once: not for a queue only
     taken up again, which waits for somebody to press play. */
  const wantsToPlay = useRef(false);
  /* Counted up each time a song is to be asked for again, the same one
     included, which is what repeating one song is. */
  const [turn, setTurn] = useState(0);
  const queueNow = useRef(queue);
  queueNow.current = queue;

  const load = useCallback(
    (next: Song, start: number, andPlay: boolean) => {
      startAt.current = start;
      offset.current = 0;
      const query = new URLSearchParams({ plays: formsPlayedHere() });
      if (start > 0) {
        query.set("start", start.toFixed(3));
      }
      audio.src = `/api/v1/music/songs/${next.id}/sound?${query.toString()}`;
      time.set({ position: Math.floor(start), length: next.seconds ?? 0 });
      if (andPlay) {
        void audio.play().catch(() => setPlaying(false));
      }
    },
    [audio],
  );

  // Another song, or the same one asked for again: its sound is asked for.
  const songId = song?.id ?? null;
  useEffect(() => {
    const now = current(queueNow.current);
    if (!now) {
      return;
    }
    const start = resumeFrom.current;
    resumeFrom.current = 0;
    load(now, start, wantsToPlay.current);
  }, [songId, turn, load]);

  // The account's choices, and with them the queue of the last visit when
  // it asked for it back.
  useEffect(() => {
    const stop = new AbortController();
    server
      .preferences(stop.signal)
      .catch(() => DEFAULT_PREFERENCES)
      .then((chosen) => {
        if (stop.signal.aborted) {
          return;
        }
        const { film_on_screen, resume_queue, max_bitrate_kbps } = chosen;
        setPreferencesHere({ film_on_screen, resume_queue, max_bitrate_kbps });
        if (resume_queue && kept) {
          resumeFrom.current = kept.position;
          setQueue({ songs: kept.songs, order: kept.order, at: kept.at, shuffle: kept.shuffle, repeat: kept.repeat });
        }
      });
    return () => stop.abort();
  }, [kept]);

  useEffect(() => {
    audio.volume = loudness.volume;
    audio.muted = loudness.muted;
    rememberLoudness(loudness);
  }, [audio, loudness]);

  // What the element says, turned into what the interface shows.
  useEffect(() => {
    const onMetadata = () => {
      const start = startAt.current;
      if (Number.isFinite(audio.duration)) {
        if (start > 0) {
          audio.currentTime = start;
        }
      } else {
        offset.current = start;
      }
      startAt.current = 0;
    };
    const onTime = () => {
      const whole = queueNow.current.songs.length > 0 ? current(queueNow.current) : null;
      time.set({
        position: Math.floor(offset.current + audio.currentTime),
        length: whole?.seconds ?? (Number.isFinite(audio.duration) ? Math.floor(audio.duration) : 0),
      });
    };
    const onPlay = () => setPlaying(true);
    const onPause = () => setPlaying(false);
    const onWaiting = () => setWaiting(true);
    const onFlowing = () => setWaiting(false);
    const onEnded = () => {
      const after = afterTheEnd(queueNow.current);
      if (!after) {
        setPlaying(false);
        return;
      }
      wantsToPlay.current = true;
      setQueue(after);
      setTurn((was) => was + 1);
    };
    const onError = () => {
      if (!audio.getAttribute("src")) {
        return;
      }
      if (import.meta.env.DEV) {
        console.debug("[music] a song could not be played", queueNow.current, audio.error);
      }
      const after = forward(queueNow.current);
      setWaiting(false);
      if (after) {
        wantsToPlay.current = true;
        setQueue(after);
        setTurn((was) => was + 1);
      } else {
        setPlaying(false);
      }
    };
    const events: [string, () => void][] = [
      ["loadedmetadata", onMetadata],
      ["timeupdate", onTime],
      ["play", onPlay],
      ["pause", onPause],
      ["waiting", onWaiting],
      ["playing", onFlowing],
      ["canplay", onFlowing],
      ["ended", onEnded],
      ["error", onError],
    ];
    for (const [name, handler] of events) {
      audio.addEventListener(name, handler);
    }
    return () => {
      for (const [name, handler] of events) {
        audio.removeEventListener(name, handler);
      }
    };
  }, [audio]);

  // The queue is kept for the next visit, and where the song has got to
  // every few seconds and as the page goes. Not before the last one was
  // taken up or let go, which would write an empty queue over it.
  const settled = preferences !== null;
  useEffect(() => {
    if (!settled) {
      return;
    }
    const keep = () =>
      rememberQueue(
        queueNow.current.songs.length > 0
          ? { ...queueNow.current, position: time.now.position }
          : null,
      );
    keep();
    const every = window.setInterval(keep, KEEP_EVERY_MS);
    window.addEventListener("pagehide", keep);
    return () => {
      window.clearInterval(every);
      window.removeEventListener("pagehide", keep);
    };
  }, [queue, settled]);

  const stop = useCallback(() => {
    audio.pause();
    audio.removeAttribute("src");
    audio.load();
    wantsToPlay.current = false;
    setPlaying(false);
    setWaiting(false);
    setOpen(false);
    setQueue(EMPTY);
    time.set({ position: 0, length: 0 });
  }, [audio]);

  // A film on the screen stops the music for good, the bar going with it,
  // or pauses it until the film is closed, as the account chose: two sounds
  // at once make sense to nobody.
  const filmOnScreen = useIsAFilmOnScreen();
  const pausedForAFilm = useRef(false);
  const onlyPause = preferences?.film_on_screen === "pause";
  useEffect(() => {
    if (filmOnScreen && queueNow.current.songs.length > 0) {
      if (!onlyPause) {
        stop();
      } else if (!audio.paused) {
        pausedForAFilm.current = true;
        audio.pause();
      }
    } else if (!filmOnScreen && pausedForAFilm.current) {
      pausedForAFilm.current = false;
      void audio.play().catch(() => setPlaying(false));
    }
  }, [filmOnScreen, onlyPause, stop, audio]);

  const preferencesNow = useRef(preferences);
  preferencesNow.current = preferences;
  const setPreferences = useCallback(async (chosen: MusicPreferences) => {
    const was = preferencesNow.current;
    setPreferencesHere(chosen);
    try {
      await server.setPreferences(chosen);
    } catch (error) {
      setPreferencesHere(was);
      throw error;
    }
  }, []);

  const music = useMemo<Music>(() => {
    /* Another place in the queue, whose song is asked for. */
    const change = (next: Queue | null, andPlay = true) => {
      if (!next) {
        return;
      }
      wantsToPlay.current = andPlay;
      setQueue(next);
      setTurn((was) => was + 1);
    };
    return {
      queue,
      song,
      playing,
      waiting,
      loudness,
      open,
      play: (songs, index, shuffle = false) => {
        if (songs.length === 0) {
          return;
        }
        change(playingFrom(songs, index, shuffle, queueNow.current.repeat));
      },
      toggle: () => {
        const now = current(queueNow.current);
        if (!now) {
          return;
        }
        if (!audio.getAttribute("src")) {
          load(now, time.now.position, true);
        } else if (audio.paused) {
          void audio.play().catch(() => setPlaying(false));
        } else {
          audio.pause();
        }
      },
      stop,
      next: () => change(forward(queueNow.current)),
      previous: () => {
        const { queue: back, restart } = backward(queueNow.current, time.now.position);
        if (restart) {
          const now = current(queueNow.current);
          if (now) {
            load(now, 0, true);
          }
        } else {
          change(back);
        }
      },
      seek: (seconds) => {
        const now = current(queueNow.current);
        if (!now) {
          return;
        }
        const to = Math.max(0, Math.min(seconds, now.seconds ?? seconds));
        if (Number.isFinite(audio.duration)) {
          audio.currentTime = to;
        } else {
          load(now, to, !audio.paused || wantsToPlay.current);
        }
        time.set({ position: Math.floor(to), length: time.now.length });
      },
      setVolume: (volume) => setLoudness((was) => ({ volume: Math.min(Math.max(volume, 0), 1), muted: volume > 0 ? false : was.muted })),
      setMuted: (muted) => setLoudness((was) => ({ ...was, muted })),
      toggleShuffle: () => setQueue((was) => withShuffle(was, !was.shuffle)),
      cycleRepeat: () => setQueue((was) => ({ ...was, repeat: nextRepeat(was.repeat) })),
      jump: (at) => change(jumpTo(queueNow.current, at)),
      playNext: (songs) => {
        const empty = queueNow.current.songs.length === 0;
        const next = playNext(queueNow.current, songs);
        if (empty) {
          change(next);
        } else {
          setQueue(next);
        }
      },
      playLast: (songs) => {
        const empty = queueNow.current.songs.length === 0;
        const next = playLast(queueNow.current, songs);
        if (empty) {
          change(next);
        } else {
          setQueue(next);
        }
      },
      remove: (at) => setQueue((was) => without(was, at)),
      setOpen,
      preferences: preferences ?? DEFAULT_PREFERENCES,
      setPreferences,
    };
  }, [queue, song, playing, waiting, loudness, open, audio, load, stop, preferences, setPreferences]);

  useMediaSession(music);

  return (
    <MusicContext.Provider value={music}>
      {children}
      <MediaSessionPosition song={song} />
    </MusicContext.Provider>
  );
}
