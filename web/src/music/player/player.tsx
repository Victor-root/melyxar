/*
 * The player of music: two elements of sound for the whole interface, made
 * once and never unmounted, so that going from page to page never stops a
 * song, taking turns so that one song follows the next without a gap. See
 * `docs/architecture/06-musique.md`.
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
import { useSettings } from "../../settings";
import { showListening } from "../../tab";
import { music as server } from "../api";
import type { MusicPreferences, Song } from "../api";
import { useMusicMarks } from "../marks";
import { formsPlayedHere } from "./forms";
import { PREPARE_AHEAD_SECONDS, fadeBetween, handOverIn, secondsLeft } from "./handover";
import { levelOf } from "./levelling";
import { countsAsListened } from "./listening";
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
import { useTellTheServer } from "./reporting";
import { MediaSessionPosition, useMediaSession } from "./session";
import { useTabs } from "./tabs";
import type { Command } from "./tabs";

export interface Music {
  queue: Queue;
  /** The song playing or paused, if there is one. */
  song: Song | null;
  playing: boolean;
  /** Waiting for the sound to arrive. */
  waiting: boolean;
  loudness: Loudness;
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
  /** How far the sound is already in hand, in seconds from the start of the
      song, which is what the bar shows in grey. */
  loaded: number;
}

const MusicContext = createContext<Music | null>(null);

/** Where the song has got to, read only by what shows it. */
const time = {
  now: { position: 0, length: 0, loaded: 0 } as Time,
  listeners: new Set<() => void>(),
  set(next: Omit<Time, "loaded"> & { loaded?: number }) {
    const loaded = next.loaded ?? 0;
    if (
      next.position !== this.now.position ||
      next.length !== this.now.length ||
      loaded !== this.now.loaded
    ) {
      this.now = { ...next, loaded };
      for (const listener of this.listeners) {
        listener();
      }
    }
  },
};

/** How far into the song the sound is in hand, from where the deck was
    started: the end of the stretch it is playing in. */
function bufferedTo(audio: HTMLAudioElement): number {
  const { buffered, currentTime } = audio;
  for (let at = 0; at < buffered.length; at += 1) {
    if (buffered.start(at) <= currentTime + 0.5 && buffered.end(at) >= currentTime) {
      return buffered.end(at);
    }
  }
  return currentTime;
}

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
  volume_mode: "track",
  crossfade_seconds: 0,
  tag_preview: true,
  spectrum: true,
  spectrum_amplitude: 100,
  skip_back_seconds: 10,
  skip_on_seconds: 10,
  hidden_tabs: [],
  longest_skip_seconds: 90,
};

/** One element of sound, asking only for what it needs to start. */
function aDeck(): HTMLAudioElement {
  const element = new Audio();
  element.preload = "metadata";
  return element;
}

/** Where the sound of a song is asked for, from `start` seconds in. */
function soundOf(song: Song, start: number): string {
  const query = new URLSearchParams({ plays: formsPlayedHere() });
  if (start > 0) {
    query.set("start", start.toFixed(3));
  }
  return `/api/v1/music/songs/${song.id}/sound?${query.toString()}`;
}

/** How often where the song has got to is written down, for a tab closed
 *  and opened again to take it up there. */
const KEEP_EVERY_MS = 5_000;

export function MusicProvider({ children }: { children: ReactNode }) {
  /* Two elements of sound, taking turns: the one playing, and the one the
     next song is made ready in, so it starts the moment the other ends, or
     fades in over it. Everything the interface does is to the one playing. */
  const [decks] = useState(() => [aDeck(), aDeck()] as const);
  const liveAt = useRef(0);
  const live = useCallback(() => decks[liveAt.current], [decks]);
  const kept = useMemo(storedQueue, []);
  /* Empty until the account has said whether a queue left in a closed tab
     is taken up again: shown and then taken away, it would be a bar that
     flickers on every visit. */
  const [queue, setQueue] = useState<Queue>(EMPTY);
  const [preferences, setPreferencesHere] = useState<MusicPreferences | null>(null);
  const [playing, setPlaying] = useState(false);
  const [waiting, setWaiting] = useState(false);
  const [loudness, setLoudness] = useState<Loudness>(storedLoudness);
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
  const turnNow = useRef(turn);
  turnNow.current = turn;
  /* The song made ready in the other element, and when it takes over. */
  const prepared = useRef<string | null>(null);
  const takeOver = useRef<number | null>(null);
  /* Set while a song that has already taken over becomes the queue's own,
     which must not ask for its sound a second time. */
  const alreadyPlaying = useRef(false);
  /* Set while one song fades into the next, which the levelling of songs
     leaves alone. */
  const fading = useRef(false);

  const load = useCallback(
    (next: Song, start: number, andPlay: boolean) => {
      const audio = live();
      startAt.current = start;
      offset.current = 0;
      audio.src = soundOf(next, start);
      time.set({ position: Math.floor(start), length: next.seconds ?? 0 });
      if (andPlay) {
        void audio.play().catch(() => setPlaying(false));
      }
    },
    [live],
  );

  /* The next song made ready, or the one made ready let go. */
  const letGoOfTheNext = useCallback(() => {
    if (takeOver.current !== null) {
      window.clearTimeout(takeOver.current);
      takeOver.current = null;
    }
    if (prepared.current !== null) {
      prepared.current = null;
      const other = decks[1 - liveAt.current];
      other.removeAttribute("src");
      other.load();
    }
  }, [decks]);

  // Another song, or the same one asked for again: its sound is asked for,
  // unless it already took over from the one before.
  const songId = song?.id ?? null;
  useEffect(() => {
    if (alreadyPlaying.current) {
      alreadyPlaying.current = false;
      return;
    }
    const now = current(queueNow.current);
    if (!now || tabs.followingNow.current) {
      return;
    }
    letGoOfTheNext();
    const start = resumeFrom.current;
    resumeFrom.current = 0;
    load(now, start, wantsToPlay.current);
  }, [songId, turn, load, letGoOfTheNext]);

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
        setPreferencesHere(chosen);
        if (chosen.resume_queue && kept && !tabs.followingNow.current) {
          resumeFrom.current = kept.position;
          setQueue({ songs: kept.songs, order: kept.order, at: kept.at, shuffle: kept.shuffle, repeat: kept.repeat });
        }
      });
    return () => stop.abort();
  }, [kept]);

  useEffect(() => {
    for (const deck of decks) {
      deck.volume = loudness.volume;
      deck.muted = loudness.muted;
    }
    rememberLoudness(loudness);
  }, [decks, loudness]);

  /* The sound goes through a gain for each element, which is what brings
     every song to the same level, the element's own volume only lowering,
     and what fades one song into the next. Made at the first play, which is
     when a browser lets a page make sound. */
  const graph = useRef<{ context: AudioContext; gains: readonly [GainNode, GainNode] } | null>(null);
  const soundThroughTheGain = useCallback(() => {
    if (graph.current) {
      if (graph.current.context.state === "suspended") {
        void graph.current.context.resume();
      }
      return;
    }
    try {
      const context = new AudioContext();
      const gains = [context.createGain(), context.createGain()] as const;
      decks.forEach((deck, at) => {
        context.createMediaElementSource(deck).connect(gains[at]).connect(context.destination);
      });
      graph.current = { context, gains };
    } catch (error) {
      if (import.meta.env.DEV) {
        console.debug("[music] songs are played without levelling", error);
      }
    }
  }, [decks]);

  const volumeMode = preferences?.volume_mode ?? DEFAULT_PREFERENCES.volume_mode;
  const volumeModeNow = useRef(volumeMode);
  volumeModeNow.current = volumeMode;
  const crossfadeNow = useRef(DEFAULT_PREFERENCES.crossfade_seconds);
  crossfadeNow.current = preferences?.crossfade_seconds ?? DEFAULT_PREFERENCES.crossfade_seconds;
  useEffect(() => {
    const through = graph.current;
    if (through && song && !fading.current) {
      through.gains[liveAt.current].gain.setTargetAtTime(levelOf(song, volumeMode), through.context.currentTime, 0.05);
    }
  }, [song, volumeMode, playing]);

  /* The next song takes over: started in the element it was made ready in,
     at its own level at once, or faded in while the one before fades out. */
  const handOver = useCallback(() => {
    takeOver.current = null;
    const after = afterTheEnd(queueNow.current);
    const next = after ? current(after) : null;
    const from = live();
    const to = decks[1 - liveAt.current];
    if (!after || !next || prepared.current !== next.id) {
      return;
    }
    const fade = fadeBetween(crossfadeNow.current, current(queueNow.current)?.seconds ?? null, next.seconds);
    const through = graph.current;
    if (through) {
      const now = through.context.currentTime;
      const out = through.gains[liveAt.current].gain;
      const into = through.gains[1 - liveAt.current].gain;
      const level = levelOf(next, volumeModeNow.current);
      out.cancelScheduledValues(now);
      into.cancelScheduledValues(now);
      if (fade > 0) {
        out.setValueAtTime(out.value, now);
        out.linearRampToValueAtTime(0, now + fade);
        into.setValueAtTime(0, now);
        into.linearRampToValueAtTime(level, now + fade);
      } else {
        into.setValueAtTime(level, now);
      }
    }
    fading.current = fade > 0 && through !== null;
    void to.play().catch(() => setPlaying(false));
    liveAt.current = 1 - liveAt.current;
    prepared.current = null;
    offset.current = 0;
    startAt.current = 0;
    alreadyPlaying.current = true;
    wantsToPlay.current = true;
    setQueue(after);
    setTurn((was) => was + 1);
    time.set({ position: 0, length: next.seconds ?? 0 });
    window.setTimeout(() => {
      from.pause();
      from.removeAttribute("src");
      from.load();
      fading.current = false;
    }, fade * 1000 + 100);
  }, [decks, live]);

  // What the element playing says, turned into what the interface shows.
  // The other one is heard only once it takes over, and only its failing to
  // load is listened to before then.
  useEffect(() => {
    const onMetadata = () => {
      const audio = live();
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
    const onLoaded = () => {
      const audio = live();
      time.set({ ...time.now, loaded: offset.current + bufferedTo(audio) });
    };
    const onTime = () => {
      const audio = live();
      const now = queueNow.current.songs.length > 0 ? current(queueNow.current) : null;
      time.set({
        position: Math.floor(offset.current + audio.currentTime),
        length: now?.seconds ?? (Number.isFinite(audio.duration) ? Math.floor(audio.duration) : 0),
        loaded: offset.current + bufferedTo(audio),
      });
      if (!now || audio.paused) {
        return;
      }
      const left = secondsLeft(audio.duration, audio.currentTime, offset.current, now.seconds);
      const after = afterTheEnd(queueNow.current);
      const next = after ? current(after) : null;
      if (left === null || !next || left > PREPARE_AHEAD_SECONDS) {
        return;
      }
      if (prepared.current !== next.id) {
        letGoOfTheNext();
        const other = decks[1 - liveAt.current];
        other.preload = "auto";
        other.src = soundOf(next, 0);
        prepared.current = next.id;
      }
      if (takeOver.current === null) {
        const fade = fadeBetween(crossfadeNow.current, now.seconds, next.seconds);
        takeOver.current = window.setTimeout(handOver, handOverIn(left, fade) * 1000);
      }
    };
    const onPlay = () => {
      soundThroughTheGain();
      setPlaying(true);
    };
    const onPause = () => {
      if (takeOver.current !== null) {
        window.clearTimeout(takeOver.current);
        takeOver.current = null;
      }
      setPlaying(false);
    };
    const onWaiting = () => setWaiting(true);
    const onFlowing = () => setWaiting(false);
    const onEnded = () => {
      const after = afterTheEnd(queueNow.current);
      if (!after) {
        setPlaying(false);
        // The last second is rounded away by the clock: a song that has
        // ended shows its whole length played.
        time.set({ ...time.now, position: time.now.length, loaded: time.now.length });
        return;
      }
      wantsToPlay.current = true;
      setQueue(after);
      setTurn((was) => was + 1);
    };
    const onError = () => {
      const audio = live();
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
      ["progress", onLoaded],
      ["play", onPlay],
      ["pause", onPause],
      ["waiting", onWaiting],
      ["playing", onFlowing],
      ["canplay", onFlowing],
      ["ended", onEnded],
      ["error", onError],
    ];
    const attached = decks.map((deck) =>
      events.map(([name, handler]) => {
        const heard = () => {
          if (deck === live()) {
            handler();
          } else if (name === "error" && deck.getAttribute("src")) {
            // The next song could not be made ready: it is asked for again,
            // the ordinary way, once the one playing ends.
            letGoOfTheNext();
          }
        };
        deck.addEventListener(name, heard);
        return [name, heard] as const;
      }),
    );
    return () => {
      decks.forEach((deck, at) => {
        for (const [name, heard] of attached[at]) {
          deck.removeEventListener(name, heard);
        }
      });
    };
  }, [decks, live, soundThroughTheGain, handOver, letGoOfTheNext]);

  // A song heard for long enough counts as listened to, once each time it
  // is played. Read as the time moves rather than drawn: nothing on the
  // screen changes for it.
  const { listened } = useMusicMarks();
  const counted = useRef<string | null>(null);
  useEffect(() => {
    const check = () => {
      const now = current(queueNow.current);
      if (!now || live().paused) {
        return;
      }
      const play = `${now.id}:${turnNow.current}`;
      if (counted.current !== play && countsAsListened(time.now.position, time.now.length)) {
        counted.current = play;
        listened(now.id);
      }
    };
    time.listeners.add(check);
    return () => {
      time.listeners.delete(check);
    };
  }, [live, listened]);

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

  /* The sound let go of, the queue left as it is. */
  const letGoOfTheSound = useCallback(() => {
    letGoOfTheNext();
    const audio = live();
    audio.pause();
    audio.removeAttribute("src");
    audio.load();
    wantsToPlay.current = false;
  }, [live, letGoOfTheNext]);

  const stop = useCallback(() => {
    letGoOfTheSound();
    setPlaying(false);
    setWaiting(false);
    setQueue(EMPTY);
    time.set({ position: 0, length: 0 });
  }, [letGoOfTheSound]);

  const pause = useCallback(() => live().pause(), [live]);
  const resume = useCallback(() => {
    void live().play().catch(() => setPlaying(false));
  }, [live]);

  /* What is done to the music in any tab is done by the one that holds the
     sound: see `tabs.ts`. */
  const stateNow = useRef({ playing, waiting, loudness });
  stateNow.current = { playing, waiting, loudness };
  const commandsNow = useRef<Record<Command, (...args: never[]) => void> | null>(null);
  const tabs = useTabs(
    {
      queue: () => queueNow.current,
      beat: () => ({ ...stateNow.current, position: time.now.position, length: time.now.length }),
      follow: (next, beat) => {
        if (next) {
          letGoOfTheSound();
          setQueue(next);
        }
        setPlaying(beat.playing);
        setWaiting(beat.waiting);
        setLoudness((was) =>
          was.volume === beat.loudness.volume && was.muted === beat.loudness.muted ? was : beat.loudness,
        );
        time.set({ position: beat.position, length: beat.length, loaded: beat.position });
      },
      letGo: letGoOfTheSound,
      obey: (name, args) => (commandsNow.current?.[name] as (...args: unknown[]) => void)(...args),
    },
    { queue, playing, waiting, loudness },
  );
  const { run } = tabs;

  // A film on the screen stops the music for good, the bar going with it,
  // or pauses it until the film is closed, as the account chose: two sounds
  // at once make sense to nobody.
  const filmOnScreen = useIsAFilmOnScreen();
  const pausedForAFilm = useRef(false);
  const onlyPause = preferences?.film_on_screen === "pause";
  useEffect(() => {
    if (filmOnScreen && queueNow.current.songs.length > 0) {
      if (!onlyPause) {
        run("stop", [], stop);
      } else if (stateNow.current.playing) {
        pausedForAFilm.current = true;
        run("pause", [], pause);
      }
    } else if (!filmOnScreen && pausedForAFilm.current) {
      pausedForAFilm.current = false;
      run("resume", [], resume);
    }
  }, [filmOnScreen, onlyPause, run, stop, pause, resume]);

  useTellTheServer(tabs.holds ? song : null, playing, stop, () => time.now.position);

  // The tab that makes the sound says so, as long as it does.
  const { t } = useSettings();
  const heard = tabs.holds && playing;
  useEffect(() => {
    showListening(heard ? t("music.tab") : null);
    return () => showListening(null);
  }, [heard, t]);

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

  const commands = useMemo(() => {
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
      play: (songs: Song[], index: number, shuffle = false) => {
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
        const audio = live();
        if (!audio.getAttribute("src")) {
          load(now, time.now.position, true);
        } else if (audio.paused) {
          void audio.play().catch(() => setPlaying(false));
        } else {
          audio.pause();
        }
      },
      pause,
      resume,
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
      seek: (seconds: number) => {
        const now = current(queueNow.current);
        if (!now) {
          return;
        }
        const to = Math.max(0, Math.min(seconds, now.seconds ?? seconds));
        const audio = live();
        if (takeOver.current !== null) {
          window.clearTimeout(takeOver.current);
          takeOver.current = null;
        }
        if (Number.isFinite(audio.duration)) {
          audio.currentTime = to;
        } else {
          load(now, to, !audio.paused || wantsToPlay.current);
        }
        time.set({ position: Math.floor(to), length: time.now.length, loaded: time.now.loaded });
      },
      setVolume: (volume: number) =>
        setLoudness((was) => ({ volume: Math.min(Math.max(volume, 0), 1), muted: volume > 0 ? false : was.muted })),
      setMuted: (muted: boolean) => setLoudness((was) => ({ ...was, muted })),
      toggleShuffle: () => setQueue((was) => withShuffle(was, !was.shuffle)),
      cycleRepeat: () => setQueue((was) => ({ ...was, repeat: nextRepeat(was.repeat) })),
      jump: (at: number) => change(jumpTo(queueNow.current, at)),
      playNext: (songs: Song[]) => {
        const empty = queueNow.current.songs.length === 0;
        const next = playNext(queueNow.current, songs);
        if (empty) {
          change(next);
        } else {
          setQueue(next);
        }
      },
      playLast: (songs: Song[]) => {
        const empty = queueNow.current.songs.length === 0;
        const next = playLast(queueNow.current, songs);
        if (empty) {
          change(next);
        } else {
          setQueue(next);
        }
      },
      remove: (at: number) => setQueue((was) => without(was, at)),
    };
  }, [live, load, stop, pause, resume]);
  commandsNow.current = commands;

  const music = useMemo<Music>(() => {
    /* A command of the interface, done by the tab that holds the sound. */
    const sent =
      <Args extends unknown[]>(name: Command, local: (...args: Args) => void) =>
      (...args: Args) =>
        run(name, args, () => local(...args));
    /* A button hands its click to what it calls: a command that asks for
       nothing is sent without it. */
    const sentBare = (name: Command, local: () => void) => () => run(name, [], local);
    return {
      queue,
      song,
      playing,
      waiting,
      loudness,
      play: sent("play", commands.play),
      toggle: sentBare("toggle", commands.toggle),
      stop: sentBare("stop", commands.stop),
      next: sentBare("next", commands.next),
      previous: sentBare("previous", commands.previous),
      seek: sent("seek", commands.seek),
      setVolume: sent("setVolume", commands.setVolume),
      setMuted: sent("setMuted", commands.setMuted),
      toggleShuffle: sentBare("toggleShuffle", commands.toggleShuffle),
      cycleRepeat: sentBare("cycleRepeat", commands.cycleRepeat),
      jump: sent("jump", commands.jump),
      playNext: sent("playNext", commands.playNext),
      playLast: sent("playLast", commands.playLast),
      remove: sent("remove", commands.remove),
      preferences: preferences ?? DEFAULT_PREFERENCES,
      setPreferences,
    };
  }, [queue, song, playing, waiting, loudness, commands, run, preferences, setPreferences]);

  useMediaSession(music);

  return (
    <MusicContext.Provider value={music}>
      {children}
      <MediaSessionPosition song={song} />
    </MusicContext.Provider>
  );
}
