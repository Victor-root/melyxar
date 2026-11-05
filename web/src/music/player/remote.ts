/*
 * The music of this device steered from another device of the account (see
 * the decision in docs/architecture/README.md).
 *
 * Every page of the device is told every order; only the one that answers
 * for the music does what it says: the tab that holds the sound, or the page
 * of the application, whose player holds it. A film on the screen answers
 * the orders itself. What starts the music is handed by the server to one
 * page of the device alone, which plays it whatever it held before.
 */

import type { RemoteOrder } from "../../api";
import type { Song } from "../api";

/** What the remote can ask of the music. */
export interface SteeredMusic {
  play: (songs: Song[], index: number, shuffle: boolean, from: number) => void;
  toggle: () => void;
  pause: () => void;
  resume: () => void;
  stop: () => void;
  next: () => void;
  previous: () => void;
  seek: (seconds: number) => void;
  setVolume: (volume: number) => void;
  setMuted: (muted: boolean) => void;
  toggleShuffle: () => void;
  cycleRepeat: () => void;
  jump: (at: number) => void;
  /** Where the song has got to, in seconds. */
  at: () => number;
}

/** Does what the order says to the music, when this page answers for it or
 *  the order starts the music. Answers whether it did anything. */
export function obeyMusic(order: RemoteOrder, music: SteeredMusic, answers: boolean): boolean {
  if (order.do === "play_songs") {
    music.play(order.songs, order.at, order.shuffle, order.from_seconds ?? 0);
    return true;
  }
  if (!answers) {
    return false;
  }
  switch (order.do) {
    case "toggle":
      music.toggle();
      return true;
    case "pause":
      music.pause();
      return true;
    case "resume":
      music.resume();
      return true;
    case "stop":
      music.stop();
      return true;
    case "next":
      music.next();
      return true;
    case "previous":
      music.previous();
      return true;
    case "seek":
      music.seek(order.seconds);
      return true;
    case "step":
      music.seek(Math.max(0, music.at() + order.seconds));
      return true;
    case "volume":
      music.setVolume(order.level);
      return true;
    case "muted":
      music.setMuted(order.muted);
      return true;
    case "shuffle":
      music.toggleShuffle();
      return true;
    case "repeat":
      music.cycleRepeat();
      return true;
    case "jump":
      music.jump(order.index);
      return true;
    // A film's tracks, and a film started: the music has nothing to do.
    case "audio":
    case "subtitle":
    case "play_work":
      return false;
  }
}
