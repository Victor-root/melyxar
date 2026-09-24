/*
 * How loud a viewer likes it, kept from one film to the next.
 *
 * A fresh video element starts at full volume, and the player builds a fresh
 * one for every film: without this, every film opened is a film opened at a
 * hundred per cent, whatever the one before was set to.
 *
 * Kept in the browser rather than on the server: it is a property of the room
 * somebody is sitting in, not of the account. The same person on a laptop in
 * the evening and on a television in the afternoon does not want one number.
 */

import { useCallback, useState } from "react";

/** A setting of the sound: how far up, and whether it is silenced outright. */
export interface Loudness {
  /** Between nothing and one, the way a video element counts it. */
  volume: number;
  muted: boolean;
}

/** What a player starts on when nothing has been remembered yet. */
export const AS_LOUD_AS_IT_GOES: Loudness = { volume: 1, muted: false };

const REMEMBERED = "melyxar.loudness";

/** Reads a stored number back, ignoring anything that is not one. */
function shareCalled(written: string | null): number | null {
  const value = Number(written);
  return written !== null && written !== "" && Number.isFinite(value)
    ? Math.min(1, Math.max(0, value))
    : null;
}

export function storedLoudness(): Loudness {
  try {
    const volume = shareCalled(window.localStorage.getItem(`${REMEMBERED}.volume`));
    if (volume === null) {
      return AS_LOUD_AS_IT_GOES;
    }
    return {
      volume,
      muted: window.localStorage.getItem(`${REMEMBERED}.muted`) === "true",
    };
  } catch {
    // A browser refusing storage plays films perfectly well, at whatever the
    // element starts on.
    return AS_LOUD_AS_IT_GOES;
  }
}

export function rememberLoudness(loudness: Loudness) {
  try {
    window.localStorage.setItem(`${REMEMBERED}.volume`, String(loudness.volume));
    window.localStorage.setItem(`${REMEMBERED}.muted`, String(loudness.muted));
  } catch {
    // The setting still holds for this sitting, which is what is being heard.
  }
}

/**
 * The loudness a trailer plays at, starting from the one kept and keeping
 * every change: the same number the films play at, so a trailer does not
 * shout at somebody who turned the films down.
 */
export function useKeptLoudness(): [Loudness, (change: Partial<Loudness>) => void] {
  const [sound, setSound] = useState(storedLoudness);
  const change = useCallback((wanted: Partial<Loudness>) => {
    setSound((was) => {
      const now = { ...was, ...wanted };
      rememberLoudness(now);
      return now;
    });
  }, []);
  return [sound, change];
}
