/*
 * What the settings screen is driven by, and nothing about how it looks.
 *
 * Two kinds of setting sit here, and they are told apart on purpose. Some are
 * kept by the server and change what it does: the fold to stereo turns a copy
 * into a rebuild, and a preferred language decides which soundtrack starts.
 * The others are kept by this browser and only change what is drawn, which is
 * why they work without asking anyone.
 *
 * Every change is sent as it is made rather than gathered behind a save
 * button: there is nothing here that is only half true while being typed, and
 * a screen of settings with an unsaved state is a screen people leave without
 * saving. What the server kept is then what is shown, because it brings a
 * value back into range rather than refusing the lot, so what it kept is not
 * always what was asked for.
 */

import { useEffect, useState } from "react";
import { api } from "../api";
import type { Library, LibraryWork, PlaybackSettings, ViewerPreferences } from "../api";
import { ApiError } from "../api";
import { useAsked } from "../asking";
import { useLibraries } from "../libraries";
import { rememberAppearance, storedAppearance } from "../player/appearance";
import type { Appearance } from "../player/appearance";

/** What went wrong last, when something did. */
export type Trouble = "unreachable" | "not_kept" | null;

const MINUTES_IN_A_DAY = 24 * 60;

function wrapIntoADay(minutes: number): number {
  return ((minutes % MINUTES_IN_A_DAY) + MINUTES_IN_A_DAY) % MINUTES_IN_A_DAY;
}

/**
 * A time of day the server keeps in universal time, in the time of this
 * browser.
 *
 * The server keeps one clock and it is UTC, which is the only one it can read
 * with certainty. Nobody should have to do that conversion in their head, so
 * it is done here, where the browser knows its own offset. What this cannot do
 * is follow the clocks changing: a time set in winter shows an hour later in
 * summer until somebody sets it again. That is said on the screen rather than
 * hidden, and it is a nightly piece of upkeep, so an hour either way costs
 * nothing.
 */
export function asLocalTime(utcMinutes: number): string {
  const local = wrapIntoADay(utcMinutes - new Date().getTimezoneOffset());
  const hours = String(Math.floor(local / 60)).padStart(2, "0");
  const minutes = String(local % 60).padStart(2, "0");
  return `${hours}:${minutes}`;
}

/** The other way, for what a time field hands back. */
export function asUtcMinutes(localTime: string): number {
  const [hours, minutes] = localTime.split(":").map(Number);
  if (!Number.isFinite(hours) || !Number.isFinite(minutes)) {
    return 0;
  }
  return wrapIntoADay(hours * 60 + minutes + new Date().getTimezoneOffset());
}

/**
 * A number a field handed back, brought inside what the server will keep.
 *
 * The bounds are here as well as on the server, so somebody dragging the
 * arrows is stopped where the server would have stopped them rather than
 * being silently corrected afterwards.
 */
export function insideTheRange(asked: number, min: number, max: number): number | null {
  if (!Number.isFinite(asked)) {
    return null;
  }
  return Math.min(max, Math.max(min, Math.round(asked)));
}

/** Everything the settings screen is handed to draw itself and be driven by. */
export interface SettingsScreen {
  /** What this viewer has decided, or nothing until the server has said. */
  kept: ViewerPreferences | null;
  change: (changes: Partial<ViewerPreferences>) => void;
  /** What the server does with every library. */
  work: LibraryWork | null;
  setWorkTo: (changes: Partial<LibraryWork>) => void;
  /** Whether wide gamut colour is ever converted, everywhere on this server:
      an administrator's own switch, not one browser's preference. */
  playback: PlaybackSettings | null;
  setPlaybackTo: (changes: Partial<PlaybackSettings>) => void;
  /** How subtitles are dressed, kept by this browser alone. */
  appearance: Appearance;
  look: (changes: Partial<Appearance>) => void;
  /** The one list the whole interface is drawn from, taken from the shell
      rather than asked for again: this is the screen that changes them, and a
      change nobody else saw would leave the bar at the top listing a library
      that is no longer there. */
  libraries: Library[];
  readLibraries: () => void;
  /** What went wrong last, for the one line that says so. */
  failed: Trouble;
}

export function useSettingsScreen(): SettingsScreen {
  const [failed, setFailed] = useState<Trouble>(null);
  const [appearance, setAppearanceState] = useState<Appearance>(storedAppearance);
  const { all: libraries, refresh: readLibraries } = useLibraries();

  const preferences = useAsked((signal) => api.preferences(signal));
  const libraryWork = useAsked((signal) => api.libraryWork(signal));
  const playbackSettings = useAsked((signal) => api.playbackSettings(signal));

  /* Held here rather than read straight out of the questions above, because
     what is shown next is what the server kept, which is not always what was
     asked for. */
  const [kept, setKept] = useState<ViewerPreferences | null>(null);
  const [work, setWork] = useState<LibraryWork | null>(null);
  const [playback, setPlayback] = useState<PlaybackSettings | null>(null);

  const fromTheServer = preferences.answer;
  useEffect(() => {
    if (fromTheServer) {
      setKept(fromTheServer);
      setFailed(null);
    }
  }, [fromTheServer]);
  useEffect(() => {
    if (libraryWork.answer) setWork(libraryWork.answer);
  }, [libraryWork.answer]);
  useEffect(() => {
    if (playbackSettings.answer) setPlayback(playbackSettings.answer);
  }, [playbackSettings.answer]);

  const couldNotBeRead =
    preferences.failure !== null || libraryWork.failure !== null || playbackSettings.failure !== null;
  useEffect(() => {
    if (couldNotBeRead) {
      setFailed("unreachable");
    }
  }, [couldNotBeRead]);

  const whyItWasNotKept = (error: unknown): Trouble =>
    error instanceof ApiError ? "not_kept" : "unreachable";

  /* Written out plainly rather than wrapped up and remembered: sending to the
     server from inside a state change would send it twice, since React runs a
     state change twice over while it is being developed to catch exactly this.
     Nothing here is watched by anything, so there is nothing to remember. */
  const setWorkTo = (changes: Partial<LibraryWork>) => {
    if (!work) {
      return;
    }
    const before = work;
    const wanted = { ...work, ...changes };
    setWork(wanted);
    api
      .setLibraryWork(wanted)
      .then((asKept) => {
        setWork(asKept);
        setFailed(null);
      })
      .catch((error) => {
        // Put back what the server still holds, rather than showing a setting
        // next to a server that never heard of it.
        setWork(before);
        setFailed(whyItWasNotKept(error));
      });
  };

  const setPlaybackTo = (changes: Partial<PlaybackSettings>) => {
    if (!playback) {
      return;
    }
    const before = playback;
    const wanted = { ...playback, ...changes };
    setPlayback(wanted);
    api
      .setPlaybackSettings(wanted)
      .then(setPlayback)
      .catch((error) => {
        setPlayback(before);
        setFailed(whyItWasNotKept(error));
      });
  };

  const change = (changes: Partial<ViewerPreferences>) => {
    setKept((before) => (before ? { ...before, ...changes } : before));
    api
      .savePreferences(changes)
      .then((answer) => {
        setKept(answer);
        setFailed(null);
      })
      .catch((error) => setFailed(whyItWasNotKept(error)));
  };

  const look = (changes: Partial<Appearance>) => {
    const next = { ...appearance, ...changes };
    setAppearanceState(next);
    rememberAppearance(next);
  };

  return {
    kept,
    change,
    work,
    setWorkTo,
    playback,
    setPlaybackTo,
    appearance,
    look,
    libraries,
    readLibraries,
    failed,
  };
}
