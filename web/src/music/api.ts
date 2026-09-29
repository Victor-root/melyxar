/*
 * What the server says of a library of music, and how it is asked.
 *
 * Kept with the rest of music rather than in the interface's own file of
 * requests: what goes wrong with music stays with music.
 */

import { get } from "../api";
import type { Picture } from "../api";

/** An artist named on an album or a song. */
export interface Credited {
  id: string;
  name: string;
}

export interface Album {
  id: string;
  library: string;
  title: string;
  /** Whose album it is. */
  artists: Credited[];
  compilation: boolean;
  year: number | null;
  songs: number;
  color: string | null;
  initial: string;
  cover: Picture[];
}

export interface Artist {
  id: string;
  library: string;
  name: string;
  initial: string;
  /** Albums that are theirs. */
  albums: number;
  /** Songs they play on. */
  songs: number;
  color: string | null;
  /** Their own picture, or the cover of their first album. */
  picture: Picture[];
}

export interface Song {
  id: string;
  title: string;
  artists: Credited[];
  album: Credited | null;
  track: number | null;
  disc: number | null;
  year: number | null;
  seconds: number | null;
  /** The file it plays from. */
  source: string | null;
}

export interface Genre {
  name: string;
  albums: number;
}

/** A letter of a list, and where its first entry stands in it. */
export interface Initial {
  letter: string;
  count: number;
  offset: number;
}

export interface Page<T> {
  items: T[];
  total: number;
}

export type AlbumOrder = "title" | "artist" | "year" | "added";
export type SongOrder = "title" | "album" | "added";

/** One album, as its own page shows it. */
export interface AlbumPage extends Album {
  /** Its songs, in their order on it. */
  tracks: Song[];
}

/** One artist, as their own page shows them. */
export interface ArtistPage extends Artist {
  their_albums: Album[];
  /** Albums of others they play on. */
  appears_on: Album[];
}

function query(values: Record<string, string | number | boolean | null | undefined>): string {
  const parts = new URLSearchParams();
  for (const [name, value] of Object.entries(values)) {
    if (value !== null && value !== undefined && value !== false) {
      parts.set(name, String(value));
    }
  }
  const written = parts.toString();
  return written ? `?${written}` : "";
}

export const music = {
  albums: (
    library: string,
    order: AlbumOrder,
    descending: boolean,
    offset: number,
    limit: number,
    narrowed: { genre?: string | null; artist?: string | null },
    signal?: AbortSignal,
  ) =>
    get<Page<Album>>(
      `/api/v1/music/${library}/albums${query({
        order,
        descending,
        offset,
        limit,
        genre: narrowed.genre,
        artist: narrowed.artist,
      })}`,
      signal,
    ),
  artists: (
    library: string,
    albumArtists: boolean,
    offset: number,
    limit: number,
    signal?: AbortSignal,
  ) =>
    get<Page<Artist>>(
      `/api/v1/music/${library}/artists${query({ album_artists: albumArtists, offset, limit })}`,
      signal,
    ),
  songs: (
    library: string,
    order: SongOrder,
    descending: boolean,
    offset: number,
    limit: number,
    signal?: AbortSignal,
  ) =>
    get<Page<Song>>(
      `/api/v1/music/${library}/songs${query({ order, descending, offset, limit })}`,
      signal,
    ),
  genres: (library: string, signal?: AbortSignal) =>
    get<Genre[]>(`/api/v1/music/${library}/genres`, signal),
  initials: (library: string, of: "albums" | "artists" | "album_artists", signal?: AbortSignal) =>
    get<Initial[]>(`/api/v1/music/${library}/initials${query({ of })}`, signal),
  album: (id: string, signal?: AbortSignal) =>
    get<AlbumPage>(`/api/v1/music/albums/${id}`, signal),
  artist: (id: string, signal?: AbortSignal) =>
    get<ArtistPage>(`/api/v1/music/artists/${id}`, signal),
};
