/*
 * What the player of music remembers in this browser: how loud, and the
 * queue, so that closing the tab and coming back finds it where it was.
 *
 * Kept in the browser, for this viewer on this device only. A browser that
 * keeps nothing, in a private window, starts over each time, which is all
 * that is lost.
 */

import type { Song } from "../api";
import type { Repeat } from "./queue";

const LOUDNESS = "melyxar.music.loudness";
const QUEUE = "melyxar.music.queue";
/* Where in the queue and in the song, kept apart from the songs: written
   every few seconds, while the songs of a whole library are a megabyte and a
   half, written only when they change. */
const PLACE = "melyxar.music.place";

export interface Loudness {
  volume: number;
  muted: boolean;
}

export interface KeptQueue {
  songs: Song[];
  order: number[];
  at: number;
  shuffle: boolean;
  repeat: Repeat;
  /** Where the song playing had got to, in seconds. */
  position: number;
}

export function storedLoudness(): Loudness {
  try {
    const read = JSON.parse(localStorage.getItem(LOUDNESS) ?? "null") as Loudness | null;
    if (read && typeof read.volume === "number" && typeof read.muted === "boolean") {
      return { volume: Math.min(Math.max(read.volume, 0), 1), muted: read.muted };
    }
  } catch {
    // Nothing kept, or nothing readable: as loud as it goes.
  }
  return { volume: 1, muted: false };
}

export function rememberLoudness(loudness: Loudness): void {
  try {
    localStorage.setItem(LOUDNESS, JSON.stringify(loudness));
  } catch {
    // A browser that keeps nothing forgets it, and that is all.
  }
}

/** Where the queue stands: the place in the order, and where the song had got
 *  to. */
export type KeptPlace = Pick<KeptQueue, "at" | "position">;

export function storedQueue(): KeptQueue | null {
  try {
    const read = JSON.parse(localStorage.getItem(QUEUE) ?? "null") as KeptQueue | null;
    if (read && Array.isArray(read.songs) && Array.isArray(read.order) && read.songs.length > 0) {
      const place = JSON.parse(localStorage.getItem(PLACE) ?? "null") as KeptPlace | null;
      return place && typeof place.at === "number" && typeof place.position === "number"
        ? { ...read, at: place.at, position: place.position }
        : read;
    }
  } catch {
    // Nothing to take up again.
  }
  return null;
}

/** The songs and their order, or nothing when the queue is empty. */
export function rememberQueue(queue: Omit<KeptQueue, "at" | "position"> | null): void {
  try {
    if (queue) {
      localStorage.setItem(QUEUE, JSON.stringify(queue));
    } else {
      localStorage.removeItem(QUEUE);
      localStorage.removeItem(PLACE);
    }
  } catch {
    // Forgotten when the tab closes, and that is all.
  }
}

export function rememberPlace(place: KeptPlace): void {
  try {
    localStorage.setItem(PLACE, JSON.stringify(place));
  } catch {
    // Forgotten when the tab closes, and that is all.
  }
}
