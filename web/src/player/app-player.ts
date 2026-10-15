/*
 * The Android application's own player, when this page runs inside it.
 *
 * The application lays this page, see-through, over a picture drawn by Media3,
 * and hands it this player under `window.MelyxarPlayer`. The page keeps doing
 * everything it does in a browser and asks the application only for what a
 * browser's video element would have done: play this address, stop, move,
 * louder. What the player does comes back through `window.melyxarPlayerTold`,
 * a few times a second and at once whenever something happens.
 *
 * Absent in a browser, where nothing here is used.
 */

import type { Shape, Turn } from "./panels";

/** What the page asks of the application's player. Plain numbers and words
 *  only: that is all that crosses from a page to an application. */
export interface AppPlayer {
  /** Plays this address in the picture numbered `id`, which replaces any
   *  other; `settings` is the JSON of {@link OpenSettings}. */
  open(id: number, address: string, settings: string): void;
  play(id: number): void;
  pause(id: number): void;
  seek(id: number, seconds: number): void;
  setSound(id: number, volume: number, muted: boolean): void;
  setRate(id: number, rate: number): void;
  setLoop(id: number, loop: boolean): void;
  /** How the picture is fitted into the screen (`auto`, `cover`, `stretch`)
   *  and how far it is turned clockwise, in degrees. */
  setShape(id: number, fit: string, turn: number): void;
  close(id: number): void;
  /** What this device decodes, as the JSON of the profile the server reads,
   *  measured by the application rather than guessed by the page. */
  profile(): string;
  pictureInPicture(): void;
  fillScreen(filled: boolean): void;
}

export interface OpenSettings {
  start: number;
  autoplay: boolean;
  volume: number;
  muted: boolean;
  rate: number;
  loop: boolean;
  fit: Shape;
  turn: Turn;
}

/** What the application's player says about the picture numbered `id`. */
export interface Told {
  id: number;
  /** Seconds. */
  position: number;
  /** Seconds, or nothing until the film's length is known. */
  duration: number | null;
  /** How far the film is held ahead, in seconds from its beginning. */
  buffered: number;
  playing: boolean;
  ended: boolean;
  seeking: boolean;
  width: number;
  height: number;
  volume: number;
  muted: boolean;
  /** Pictures shown and pictures dropped since it opened. */
  shown: number;
  dropped: number;
  /** Why it stopped, when it could not play. */
  error: string | null;
  /** What happened since it last spoke, named as a video element names it:
   *  `loadedmetadata`, `playing`, `pause`, `seeked`, `waiting`, `ended`,
   *  `error`. */
  events: string[];
}

declare global {
  interface Window {
    MelyxarPlayer?: AppPlayer;
    melyxarPlayerTold?: (told: Told) => void;
  }
}

/** The application's player, or nothing in a browser. */
export const appPlayer: AppPlayer | null =
  typeof window !== "undefined" ? (window.MelyxarPlayer ?? null) : null;

const listening = new Map<number, (told: Told) => void>();

if (appPlayer) {
  window.melyxarPlayerTold = (told) => listening.get(told.id)?.(told);
}

/** Hears what the player says of the picture numbered `id`, until the
 *  returned function is called. */
export function hear(id: number, heard: (told: Told) => void): () => void {
  listening.set(id, heard);
  return () => {
    listening.delete(id);
  };
}
