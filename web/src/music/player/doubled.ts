/*
 * Two songs heard at once: both decks of the application's engine sounding
 * together for longer than the fade between them allows, which nothing else
 * would ever say (see the music journal of the server).
 */

import type { DeckTold } from "./app-music";
import type { DecksHeard } from "../api";

/** How long both decks may sound together past the fade before it is said:
 *  the engine speaks every quarter of a second, and the deck let go of
 *  after a hand over is stopped a little after its fade. */
export const TOGETHER_GRACE_SECONDS = 2;

/** Since when both decks have been sounding, in milliseconds, and whether
 *  it was said already. */
export interface Together {
  since: number | null;
  said: boolean;
}

export const APART: Together = { since: null, said: false };

/** Where both decks stand now, and whether that is to be said: once for
 *  each stretch of both sounding, when it outlasts the fade. */
export function together(
  was: Together,
  both: boolean,
  nowMs: number,
  fadeSeconds: number,
): { now: Together; seconds: number; say: boolean } {
  if (!both) {
    return { now: APART, seconds: 0, say: false };
  }
  const since = was.since ?? nowMs;
  const seconds = (nowMs - since) / 1000;
  const say = !was.said && seconds > fadeSeconds + TOGETHER_GRACE_SECONDS;
  return { now: { since, said: was.said || say }, seconds, say };
}

/** The two decks as the journal is told of them. */
export function decksHeard(
  zero: DeckTold,
  one: DeckTold,
  liveDeck: number,
  fadeSeconds: number,
  togetherSeconds: number,
): DecksHeard {
  return {
    live_deck: liveDeck,
    fade_seconds: fadeSeconds,
    deck0_second: zero.position,
    deck0_length: zero.duration,
    deck0_sounding: zero.advancing,
    deck1_second: one.position,
    deck1_length: one.duration,
    deck1_sounding: one.advancing,
    together_seconds: togetherSeconds,
  };
}
