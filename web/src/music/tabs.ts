/*
 * The tabs of a library of music, which of them an account shows, and the
 * one beside another.
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

/** The tab beside the open one, that way, or nothing at either end. */
export function tabBeside(shown: MusicTab[], open: MusicTab, way: -1 | 1): MusicTab | null {
  return shown[shown.indexOf(open) + way] ?? null;
}
