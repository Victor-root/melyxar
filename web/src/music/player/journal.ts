/*
 * What the player of music tells the server's journal of itself: the sound
 * of its two decks, its queue, the presses and the tab holding the sound,
 * each under a tag of its own there. Nothing waits on any of it.
 */

import { music as server } from "../api";
import type { MusicSaw } from "../api";

/** Tells the journal, without waiting. */
export function tellTheJournal(saw: MusicSaw): void {
  void server.tellTheJournalOfMusic(saw);
}

/** The song an address of the sound of one is for, or nothing. */
export function songOfAddress(address: string | null): string | null {
  return address?.match(/\/music\/songs\/([^/?]+)\/sound/)?.[1] ?? null;
}

let onTheCard = false;

/** Does what a press on the card of the lock screen, a headset or a watch
 *  asks, said to come from there. */
export function pressedOnTheCard(press: () => void): void {
  onTheCard = true;
  try {
    press();
  } finally {
    onTheCard = false;
  }
}

/** Where the press being done comes from. */
export function pressedFrom(): "interface" | "card" {
  return onTheCard ? "card" : "interface";
}
