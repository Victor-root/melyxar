/*
 * What the settings are driven by, the server's and somebody's own, and
 * nothing about how they look.
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
import type { LibraryWork, PlaybackSettings, ViewerPreferences } from "../api";
import { ApiError } from "../api";
import { useAsked } from "../asking";
import { rememberAppearance, storedAppearance } from "../player/appearance";
import type { Appearance } from "../player/appearance";

/** What went wrong last, when something did. */
export type Trouble = "unreachable" | "not_kept" | null;

function whyItWasNotKept(error: unknown): Trouble {
  return error instanceof ApiError ? "not_kept" : "unreachable";
}

/** A group of settings the server keeps, and the way to change it. */
export interface Kept<T> {
  /** What the server holds, or nothing until it has said. */
  kept: T | null;
  setTo: (changes: Partial<T>) => void;
  /** What went wrong last, for the one line that says so. */
  failed: Trouble;
}

/**
 * A group of settings the server keeps whole: read once, and written back
 * whole with the change in it.
 */
export function useKept<T>(
  read: (signal: AbortSignal) => Promise<T>,
  write: (value: T) => Promise<T>,
): Kept<T> {
  const asked = useAsked(read);
  const [kept, setKept] = useState<T | null>(null);
  const [failed, setFailed] = useState<Trouble>(null);

  const answer = asked.answer;
  useEffect(() => {
    if (answer) {
      setKept(answer);
    }
  }, [answer]);

  const couldNotBeRead = asked.failure !== null;
  useEffect(() => {
    if (couldNotBeRead) {
      setFailed("unreachable");
    }
  }, [couldNotBeRead]);

  /* Written out plainly rather than wrapped up and remembered: sending to the
     server from inside a state change would send it twice, since React runs a
     state change twice over while it is being developed to catch exactly
     this. */
  const setTo = (changes: Partial<T>) => {
    if (!kept) {
      return;
    }
    const before = kept;
    const wanted = { ...kept, ...changes };
    setKept(wanted);
    write(wanted)
      .then((asKept) => {
        setKept(asKept);
        setFailed(null);
      })
      .catch((error) => {
        // Put back what the server still holds, rather than showing a setting
        // next to a server that never heard of it.
        setKept(before);
        setFailed(whyItWasNotKept(error));
      });
  };

  return { kept, setTo, failed };
}

/** What the server does with every library. */
export function useLibraryWork(): Kept<LibraryWork> {
  return useKept(api.libraryWork, api.setLibraryWork);
}

/** Whether wide gamut colour is ever converted, everywhere on this server. */
export function usePlaybackSettings(): Kept<PlaybackSettings> {
  return useKept(api.playbackSettings, api.setPlaybackSettings);
}

/** What this viewer has decided, sent a change at a time. */
export interface Preferences {
  kept: ViewerPreferences | null;
  change: (changes: Partial<ViewerPreferences>) => Promise<void>;
  failed: Trouble;
}

export function usePreferences(): Preferences {
  const asked = useAsked((signal) => api.preferences(signal));
  const [kept, setKept] = useState<ViewerPreferences | null>(null);
  const [failed, setFailed] = useState<Trouble>(null);

  const answer = asked.answer;
  useEffect(() => {
    if (answer) {
      setKept(answer);
      setFailed(null);
    }
  }, [answer]);

  const couldNotBeRead = asked.failure !== null;
  useEffect(() => {
    if (couldNotBeRead) {
      setFailed("unreachable");
    }
  }, [couldNotBeRead]);

  const change = (changes: Partial<ViewerPreferences>): Promise<void> => {
    setKept((before) => (before ? { ...before, ...changes } : before));
    return api
      .savePreferences(changes)
      .then((answer) => {
        setKept(answer);
        setFailed(null);
      })
      .catch((error) => setFailed(whyItWasNotKept(error)));
  };

  return { kept, change, failed };
}

/** How subtitles are dressed, kept by this browser alone. */
export function useSubtitleLook(): [Appearance, (changes: Partial<Appearance>) => void] {
  const [appearance, setAppearance] = useState<Appearance>(storedAppearance);
  const look = (changes: Partial<Appearance>) => {
    const next = { ...appearance, ...changes };
    setAppearance(next);
    rememberAppearance(next);
  };
  return [appearance, look];
}
