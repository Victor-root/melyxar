/*
 * What brings each deck to its level and fades one song into the next.
 *
 * In a browser, a gain for each element of sound, made at the first play,
 * which is when a browser lets a page make sound. In the Android application,
 * the engine's own gain on each of its players, which can raise a song as a
 * browser's gain can, and keeps fading in the background on time.
 */

import { appMusic } from "./app-music";
import type { AppMusic } from "./app-music";
import type { Deck } from "./deck";

export interface Mixer {
  /** Brings a deck to its level, gently. */
  level(deck: number, level: number): void;
  /** Woken for a play, when what makes the sound was put to sleep. */
  wake(): void;
}

/** A browser's, which also fades one song into the next: the engine does
 *  that itself, as it hands one deck over to the other. */
export interface BrowserMixer extends Mixer {
  /** Fades `from` out and `to` in to `level` over `seconds`, or puts `to` at
   *  its level at once when there are none. */
  cross(from: number, to: number, seconds: number, level: number): void;
}

/** How long a deck takes to reach a new level outside a fade. */
const SETTLING_SECONDS = 0.05;

/** The gains of a browser, over its elements of sound, or nothing when the
 *  browser will not make them. */
export function browserMixer(decks: readonly Deck[]): BrowserMixer | null {
  try {
    const context = new AudioContext();
    const gains = decks.map((deck) => {
      const gain = context.createGain();
      context.createMediaElementSource(deck as HTMLAudioElement).connect(gain).connect(context.destination);
      return gain;
    });
    return {
      level: (deck, level) => gains[deck].gain.setTargetAtTime(level, context.currentTime, SETTLING_SECONDS),
      cross: (from, to, seconds, level) => {
        const now = context.currentTime;
        const out = gains[from].gain;
        const into = gains[to].gain;
        out.cancelScheduledValues(now);
        into.cancelScheduledValues(now);
        if (seconds > 0) {
          out.setValueAtTime(out.value, now);
          out.linearRampToValueAtTime(0, now + seconds);
          into.setValueAtTime(0, now);
          into.linearRampToValueAtTime(level, now + seconds);
        } else {
          into.setValueAtTime(level, now);
        }
      },
      wake: () => {
        if (context.state === "suspended") {
          void context.resume();
        }
      },
    };
  } catch (error) {
    if (import.meta.env.DEV) {
      console.debug("[music] songs are played without levelling", error);
    }
    return null;
  }
}

/** The engine's gains, in the application, or nothing in a browser. */
export const engineMixer: Mixer | null = appMusic ? engineGains(appMusic) : null;

function engineGains(engine: AppMusic): Mixer {
  return {
    level: (deck, level) => engine.level(deck, level, SETTLING_SECONDS),
    // The engine never sleeps while it holds a song.
    wake: () => {},
  };
}
