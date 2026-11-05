/*
 * The film of this device steered from another device of the account (see
 * the decision in docs/architecture/README.md): every order that has to do
 * with what is on the screen is done by the film, the music being left alone
 * while one is.
 */

import type { RemoteOrder } from "../api";

/** What the remote can ask of the film on the screen. */
export interface SteeredFilm {
  playing: boolean;
  playOrPause: () => void;
  goTo: (seconds: number) => void;
  stepBy: (seconds: number) => void;
  leave: () => void;
  /** Absent for a film with no episode after or before it. */
  next?: () => void;
  previous?: () => void;
  setLoudness: (volume: number) => void;
  setMuted: (off: boolean) => void;
  audioId: string | null;
  subtitleId: string | null;
  choose: (audio: string | null, subtitle: string | null) => void;
}

/** Does what the order says to the film. Answers whether it did anything. */
export function obeyFilm(order: RemoteOrder, film: SteeredFilm): boolean {
  switch (order.do) {
    case "toggle":
      film.playOrPause();
      return true;
    case "pause":
    case "resume":
      if (film.playing === (order.do === "pause")) {
        film.playOrPause();
      }
      return true;
    case "stop":
      film.leave();
      return true;
    case "seek":
      film.goTo(order.seconds);
      return true;
    case "step":
      film.stepBy(order.seconds);
      return true;
    case "next":
      film.next?.();
      return film.next !== undefined;
    case "previous":
      film.previous?.();
      return film.previous !== undefined;
    case "volume":
      film.setLoudness(order.level);
      return true;
    case "muted":
      film.setMuted(order.muted);
      return true;
    case "audio":
      film.choose(order.track, film.subtitleId);
      return true;
    case "subtitle":
      film.choose(film.audioId, order.track);
      return true;
    // A queue of songs means nothing to a film; what starts something is
    // done by the page.
    case "shuffle":
    case "repeat":
    case "jump":
    case "play_work":
    case "play_songs":
      return false;
  }
}
