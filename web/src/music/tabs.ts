/*
 * The tabs of a library of music, which of them an account shows, and the
 * swipe that moves a phone from one to the next.
 */

export const MUSIC_TABS = [
  "for_you",
  "albums",
  "album_artists",
  "artists",
  "songs",
  "playlists",
  "favourites",
  "genres",
] as const;

export type MusicTab = (typeof MUSIC_TABS)[number];

/** The tabs an account shows, in their order. Never none: a library with no
 *  tab to open would show nothing, so a choice that hides every one hides
 *  none, as the server keeps it. */
export function shownTabs(hidden: string[]): MusicTab[] {
  const shown = MUSIC_TABS.filter((tab) => !hidden.includes(tab));
  return shown.length > 0 ? shown : [...MUSIC_TABS];
}

/** The tab a library opens on: the one asked for when it is shown, and
 *  otherwise the first that is. */
export function openTab(asked: string | null, shown: MusicTab[]): MusicTab {
  return shown.find((tab) => tab === asked) ?? shown[0];
}

/** How far a finger has to travel sideways to move to another tab, and how
 *  much more sideways than up or down: a scroll that drifts is not a swipe. */
const SWIPE_ACROSS = 56;
const SWIPE_SLANT = 1.6;

/** Which way a finger that moved by `dx` and `dy` turns the tabs: 1 to the
 *  next one, the finger having gone left, -1 to the one before, and 0 for no
 *  swipe at all. */
export function swipeOf(dx: number, dy: number): -1 | 0 | 1 {
  if (Math.abs(dx) < SWIPE_ACROSS || Math.abs(dx) < Math.abs(dy) * SWIPE_SLANT) {
    return 0;
  }
  return dx < 0 ? 1 : -1;
}

/** The tab beside the open one, that way, or nothing at either end. */
export function tabBeside(shown: MusicTab[], open: MusicTab, way: -1 | 1): MusicTab | null {
  return shown[shown.indexOf(open) + way] ?? null;
}
