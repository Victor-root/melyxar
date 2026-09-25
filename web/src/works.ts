/*
 * What a card or a page may offer, decided in one place.
 *
 * The card, its menu and the page of a work each ask the same questions, and
 * two answers written apart are two answers that drift: a photo offered a
 * play button on one of them and not on the other.
 */

import type { Card } from "./api";

/** Whether a card starts something playing rather than only opening a page.
 *  A series plays the episode it carries on with, which its own page works
 *  out; a folder is opened to see what it holds, and a photo is looked at
 *  rather than played. */
export function playsOnItsOwn(card: Pick<Card, "source" | "kind">): boolean {
  return card.kind === "series" || (card.source !== null && card.kind !== "photo");
}

/** Whether a work has its name, whoever gave it: a provider, a person, or
 *  the file of something somebody filmed or photographed themselves. */
export function isNamed(identification: Card["identification"]): boolean {
  return identification === "identified" || identification === "manual" || identification === "own";
}

/** Whether a catalogue could say anything about it. What somebody filmed or
 *  photographed themselves is in none, so nothing offers to look it up or to
 *  choose among the pictures a provider holds. */
export function isCatalogued(identification: Card["identification"]): boolean {
  return identification !== "own";
}
