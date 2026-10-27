/*
 * The Android application's engine of sound, when this page runs inside it.
 *
 * The player of music keeps its queue, its rules and everything it tells the
 * server, and hands the application only the sound: its two decks are two
 * players of the application, held by a service that keeps playing with the
 * screen off or the application left (see the decision in
 * docs/architecture/README.md). What cannot wait for a page in the background
 * is done there too: the next song taking over right at the end of the one
 * before, and the level and the fades of each deck. The card of the lock
 * screen and of the notifications shows what the page describes, and every
 * press on it comes back here as a press of the page's own buttons.
 *
 * Absent in a browser, where nothing here is used.
 */

/** What the page asks of the engine. Plain numbers and words only. */
export interface AppMusic {
  /** Plays this address in `deck`, at once or once asked. */
  load(deck: number, address: string, andPlay: boolean): void;
  /** Lets go of what `deck` holds. */
  clear(deck: number): void;
  play(deck: number): void;
  pause(deck: number): void;
  seek(deck: number, seconds: number): void;
  setSound(deck: number, volume: number, muted: boolean): void;
  /** Brings `deck` to this factor of its sound, which may be above one, over
   *  this many seconds. */
  level(deck: number, level: number, seconds: number): void;
  /** When `from` reaches `atSeconds` of its own clock, starts `to`, fading
   *  the one out and the other in to `level` over `fadeSeconds`, and says
   *  so with a `handedover` on `to`. Replaces any hand over asked before. */
  handOverAt(from: number, to: number, atSeconds: number, fadeSeconds: number, level: number): void;
  cancelHandOver(): void;
  /** What the card of the lock screen shows, as the JSON of a
   *  {@link Described}, or nothing to take it away. */
  describe(described: string | null): void;
  /** The forms of sound the device plays, as the server reads them:
   *  `mp3,aac,flac`. */
  forms(): string;
}

/** The song the card shows, and what its buttons can do. */
export interface Described {
  title: string;
  artist: string;
  album: string;
  /** The address of the cover, whole, or nothing. */
  artwork: string | null;
  seconds: number | null;
  /** Whether there is a song to go on to. Going back always is: a few
   *  seconds in, it is the same song from its start. */
  canStepOn: boolean;
}

/** What the engine says of one deck. */
export interface DeckTold {
  deck: number;
  /** Which song given to the deck this is about, counted up at each one
   *  given or let go of. */
  given: number;
  /** Seconds, on the deck's own clock. */
  position: number;
  /** Seconds, or nothing when what it plays has no known length. */
  duration: number | null;
  /** How far the sound is in hand, in seconds on the same clock. */
  buffered: number;
  /** Not paused, as an element of sound means it. */
  playing: boolean;
  /** Whether the clock is moving right now. */
  advancing: boolean;
  /** Why it stopped, when it could not play. */
  error: string | null;
  /** What happened since it last spoke, named as an element of sound names
   *  it, and `handedover` when it has just taken over from the other. */
  events: string[];
}

/** A press on the card, a headset or a watch, for the page to answer. */
export type Asked = "play" | "pause" | "next" | "previous" | "stop" | "seek";

declare global {
  interface Window {
    MelyxarMusic?: AppMusic;
    melyxarMusicTold?: (told: DeckTold) => void;
    melyxarMusicAsked?: (asked: Asked, seconds: number) => void;
  }
}

/** The application's engine, or nothing in a browser. */
export const appMusic: AppMusic | null =
  typeof window !== "undefined" ? (window.MelyxarMusic ?? null) : null;

const listening = new Map<number, (told: DeckTold) => void>();
let watching: ((told: DeckTold) => void) | null = null;

if (appMusic) {
  window.melyxarMusicTold = (told) => {
    listening.get(told.deck)?.(told);
    watching?.(told);
  };
}

/** Hears what the engine says of `deck`. */
export function hearDeck(deck: number, heard: (told: DeckTold) => void): void {
  listening.set(deck, heard);
}

/** Hears what the engine says of either deck, after the deck itself, or
 *  nothing more. */
export function watchDecks(heard: ((told: DeckTold) => void) | null): void {
  watching = heard;
}
