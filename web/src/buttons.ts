/*
 * The buttons grouped at the right end of the bar at the top: in which order,
 * and which of them are moved into the account's menu next to them.
 */

import type { HeaderButton } from "./api";

/** Every button, in the order the bar shows them until somebody chooses. */
export const EVERY_HEADER_BUTTON: HeaderButton[] = [
  "search",
  "notifications",
  "favourites",
  "watch_later",
];

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

/** The order of the bar, always holding every button once: one never
 *  placed goes after the others, where everybody starts with it. */
export function buttonOrder(names: readonly string[]): HeaderButton[] {
  return knownButtons([...names, ...EVERY_HEADER_BUTTON]);
}
