/*
 * The buttons of the bar at the top: in which order, and which of them are on
 * the bar rather than in the account's menu next to it.
 */

import type { HeaderButton } from "./api";

/** Every button, in the order everybody starts with. */
export const EVERY_HEADER_BUTTON: HeaderButton[] = [
  "search",
  "favourites",
  "watch_later",
  "notifications",
  "scan",
  "administration",
  "cast",
  "settings",
];

/** The buttons on the bar until somebody chooses; the others are in the
 *  menu. */
export const IN_THE_BAR_AT_FIRST: HeaderButton[] = [
  "search",
  "favourites",
  "watch_later",
  "notifications",
];

/** What only an administrator has any use for, offered to nobody else. */
export const FOR_ADMINISTRATORS: HeaderButton[] = ["scan", "administration"];

/** Buttons read from a list that may be old, doubled or partial: each known
 *  one once, the unknown dropped. */
export function knownButtons(names: readonly string[]): HeaderButton[] {
  const read: HeaderButton[] = [];
  for (const name of names) {
    const button = EVERY_HEADER_BUTTON.find((one) => one === name);
    if (button && !read.includes(button)) {
      read.push(button);
    }
  }
  return read;
}

/** The order of the buttons, always holding every one once: one never
 *  placed goes after the others, where everybody starts with it. */
export function buttonOrder(names: readonly string[]): HeaderButton[] {
  return knownButtons([...names, ...EVERY_HEADER_BUTTON]);
}
