/*
 * The songs of an album cut into its discs, and how long it runs.
 */

import type { Song } from "./api";

export interface Disc {
  /** Its number, or nothing for songs that do not say. */
  number: number | null;
  songs: Song[];
}

/**
 * The songs of an album, disc by disc, in the order they came. An album on
 * one disc is one disc, whatever its songs say of it: a heading reading
 * "Disc 1" over the only one there is tells nobody anything.
 */
export function byDisc(songs: Song[]): Disc[] {
  const discs: Disc[] = [];
  for (const song of songs) {
    const last = discs[discs.length - 1];
    if (last && last.number === song.disc) {
      last.songs.push(song);
    } else {
      discs.push({ number: song.disc, songs: [song] });
    }
  }
  return discs.length > 1 ? discs : [{ number: null, songs }];
}

/** How long songs run together, in whole minutes, rounded. */
export function minutesOf(songs: Song[]): number {
  const seconds = songs.reduce((sum, song) => sum + (song.seconds ?? 0), 0);
  return Math.round(seconds / 60);
}
