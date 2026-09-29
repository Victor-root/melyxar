/*
 * A song as the tests of music make one: nothing about it but what a test
 * gives, written once for all of them.
 */

import type { Song } from "./api";

export function aSong(changes: Partial<Song> = {}): Song {
  return {
    id: changes.title ?? "song",
    title: "song",
    artists: [],
    album: null,
    track: null,
    disc: null,
    year: null,
    seconds: 200,
    source: null,
    cover: [],
    lufs: null,
    peak_dbfs: null,
    album_lufs: null,
    ...changes,
  };
}
