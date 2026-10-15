/*
 * One deck of the player of music: what it asks of the thing a song plays in.
 *
 * In a browser that is an element of sound. In the Android application it is
 * one of the two players of the application's engine (see app-music.ts),
 * which this answers for with the same words, so the player never knows
 * which of the two it has.
 */

import { appMusic, hearDeck } from "./app-music";
import type { AppMusic, DeckTold } from "./app-music";

/** Everything the player reads, sets and listens to on a deck. */
export interface Deck extends EventTarget {
  src: string;
  preload: string;
  currentTime: number;
  readonly duration: number;
  readonly paused: boolean;
  volume: number;
  muted: boolean;
  readonly buffered: TimeRanges;
  readonly error: MediaError | null;
  play(): Promise<void>;
  pause(): void;
  load(): void;
  getAttribute(name: "src"): string | null;
  removeAttribute(name: "src"): void;
}

/** One deck, asking only for what it needs to start: the element of a
 *  browser, or deck `at` of the application's engine. */
export function aDeck(at: number): Deck {
  if (appMusic) {
    return new NativeDeck(at, appMusic);
  }
  const element = new Audio();
  element.preload = "metadata";
  return element;
}

/** What a deck knows before the engine has said anything. */
const UNTOLD: Omit<DeckTold, "deck"> = {
  position: 0,
  duration: null,
  buffered: 0,
  playing: false,
  advancing: false,
  error: null,
  events: [],
};

class NativeDeck extends EventTarget implements Deck {
  preload = "metadata";
  private address: string | null = null;
  private told: Omit<DeckTold, "deck"> = UNTOLD;
  /** When the engine last said where the deck is, to carry its clock on
   *  between two of its words. */
  private toldAt = performance.now();
  /** Asked to play, and not yet heard playing or stopping. */
  private wanted = false;
  /** Set until the address set last is handed over. */
  private handing = false;
  private loudness = { volume: 1, muted: false };

  constructor(
    private readonly at: number,
    private readonly engine: AppMusic,
  ) {
    super();
    hearDeck(at, (told) => this.heard(told));
  }

  get src() {
    return this.address ?? "";
  }

  /** Handed over once whoever set it is done: the player asks to play in
   *  the same moment, after the address. */
  set src(address: string) {
    this.address = address;
    this.told = UNTOLD;
    this.wanted = false;
    this.handing = true;
    queueMicrotask(() => {
      if (this.address === address) {
        this.handing = false;
        this.engine.load(this.at, new URL(address, window.location.href).href, this.wanted);
      }
    });
  }

  getAttribute(): string | null {
    return this.address;
  }

  removeAttribute() {
    this.address = null;
    this.handing = false;
    this.wanted = false;
    this.told = UNTOLD;
  }

  load() {
    if (this.address === null) {
      this.engine.clear(this.at);
    }
  }

  private heard(told: DeckTold) {
    const before = this.told;
    this.told = told;
    this.toldAt = performance.now();
    // The engine has caught up with what was asked of it.
    if (told.events.some((name) => name === "play" || name === "pause" || name === "ended")) {
      this.wanted = false;
    }
    if (told.buffered !== before.buffered) {
      this.dispatchEvent(new Event("progress"));
    }
    for (const name of told.events) {
      this.dispatchEvent(new Event(name));
    }
    this.dispatchEvent(new Event("timeupdate"));
  }

  get currentTime() {
    const { position, advancing, duration } = this.told;
    if (!advancing) {
      return position;
    }
    const carried = position + (performance.now() - this.toldAt) / 1000;
    return duration === null ? carried : Math.min(duration, carried);
  }

  set currentTime(seconds: number) {
    this.told = { ...this.told, position: seconds };
    this.toldAt = performance.now();
    this.engine.seek(this.at, seconds);
  }

  get duration() {
    return this.told.duration ?? Number.NaN;
  }

  get paused() {
    return !this.told.playing && !this.wanted;
  }

  get buffered(): TimeRanges {
    const held = this.told.buffered;
    return { length: held > 0 ? 1 : 0, start: () => 0, end: () => held };
  }

  get volume() {
    return this.loudness.volume;
  }

  set volume(volume: number) {
    this.loudness = { ...this.loudness, volume };
    this.engine.setSound(this.at, volume, this.loudness.muted);
  }

  get muted() {
    return this.loudness.muted;
  }

  set muted(muted: boolean) {
    this.loudness = { ...this.loudness, muted };
    this.engine.setSound(this.at, this.loudness.volume, muted);
  }

  get error(): MediaError | null {
    const message = this.told.error;
    return message === null
      ? null
      : {
          code: MediaError.MEDIA_ERR_DECODE,
          message,
          MEDIA_ERR_ABORTED: MediaError.MEDIA_ERR_ABORTED,
          MEDIA_ERR_NETWORK: MediaError.MEDIA_ERR_NETWORK,
          MEDIA_ERR_DECODE: MediaError.MEDIA_ERR_DECODE,
          MEDIA_ERR_SRC_NOT_SUPPORTED: MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED,
        };
  }

  play() {
    if (this.address === null) {
      return Promise.reject(new Error("nothing to play"));
    }
    this.wanted = true;
    // Asked along with the address when that is still to be handed over.
    if (!this.handing) {
      this.engine.play(this.at);
    }
    return Promise.resolve();
  }

  pause() {
    this.wanted = false;
    this.engine.pause(this.at);
  }
}
