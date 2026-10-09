/*
 * What the server says of a library of music, and how it is asked.
 *
 * Kept with the rest of music rather than in the interface's own file of
 * requests: what goes wrong with music stays with music.
 */

import { get, getRaw, post, put, remove } from "../api";
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
  /** The cover of its album. */
  cover: Picture[];
  /** How loud it is, in LUFS, and how high its sound reaches, in dBFS,
      once measured. */
  lufs: number | null;
  peak_dbfs: number | null;
  /** How loud its whole album is. */
  album_lufs: number | null;
}

export interface Genre {
  name: string;
  albums: number;
  /** The albums it is shown by: the last to arrive, those with a cover first. */
  shown: GenreAlbum[];
}

export interface GenreAlbum {
  id: string;
  color: string | null;
  cover: Picture[];
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

/** What a few words found in the music, a few of each. */
export interface Found {
  albums: Album[];
  artists: Artist[];
  songs: Song[];
}

/** What an account chose for its music. */
export interface MusicPreferences {
  film_on_screen: "stop" | "pause";
  resume_queue: boolean;
  /** Whether the bar of the player closes when the last song has ended. */
  close_when_done: boolean;
  /** Nothing for every song as it is. */
  max_bitrate_kbps: number | null;
  volume_mode: VolumeMode;
  /** How many seconds one song fades into the next, nought for none. */
  crossfade_seconds: number;
  /** Whether the tag manager shows what is about to change first. */
  tag_preview: boolean;
  /** Whether a wave of the sound plays behind the bar of the player. */
  spectrum: boolean;
  /** How high the wave rises, from nought to a hundred. */
  spectrum_amplitude: number;
  /** How far the buttons that skip back and on move within a song. */
  skip_back_seconds: number;
  skip_on_seconds: number;
  /** The tabs of a library of music this account hides, by name. */
  hidden_tabs: string[];
  /** The longest a skip can be, as the server keeps it. */
  longest_skip_seconds: number;
}

/** How songs are levelled: not at all, each to the same level, or each
 *  album to the same level with its songs kept apart. */
export type VolumeMode = "off" | "track" | "album";

/** How the sound of a song is spread: `bands` levels for each of the
 *  `framesASecond` readings of every second, from nought for nothing to 255 for
 *  as loud as the song gets. */
export interface SongSpectrum {
  bands: number;
  framesASecond: number;
  levels: Uint8Array;
}

/** One line sung at a known moment. */
export interface LyricLine {
  at_ms: number;
  text: string;
}

/** The words of a song, and where they were found. */
export interface SongLyrics {
  source: "song" | "beside" | "online";
  plain: string;
  /** Empty when the words carry no moments. */
  lines: LyricLine[];
  instrumental: boolean;
  /** What came of lining the lines up with the song, and how they were moved
      if they were. */
  synchronised: {
    conclusion: "aligned" | "already_fits" | "not_sure" | "too_few_lines" | "too_long";
    shift_ms: number;
    lines_moved: number;
    confidence: number;
  } | null;
}

/** One entry of LRCLIB offered for a song, to be taken as its words. */
export interface LyricsOffer {
  id: number;
  artist: string;
  title: string;
  album: string | null;
  seconds: number | null;
  /** Stamped line by line, so it follows the song. */
  synced: boolean;
  /** Whether it has words at all. */
  plain: boolean;
  instrumental: boolean;
  /** The lines that carry a moment, and the longest stretch without one, in
      seconds: where the song runs on with nothing lit. */
  synced_lines: number;
  longest_gap_seconds: number | null;
}

/** One fact the page saw of the lyrics playing, for the journal. The server
    names the facts: see its `lyrics_watch` module for the whole list. */
export type LyricsSaw = { song: string } & (
  | {
      saw: "lyrics_read";
      source: SongLyrics["source"];
      lines: number;
      first_at_ms: number | null;
      last_at_ms: number | null;
      out_of_order: number;
      empty_lines: number;
      instrumental: boolean;
    }
  | {
      saw: "line_lit";
      why: "first" | "played" | "after_a_jump";
      position_ms: number;
      active: number;
      lines: number;
      line_at_ms: number | null;
      next_at_ms: number | null;
      text: string;
    }
  | { saw: "line_not_lit"; position_ms: number; expected: number; shown: number; expected_at_ms: number | null }
  | { saw: "clock_stood_still"; position_ms: number; for_ms: number }
);

/** What a library of music does beyond the rest. */
export interface MusicLibraryOptions {
  lyrics_online: boolean;
  /** Whether the tag manager may write into its files. */
  tag_writing: boolean;
  /** Whether the covers its albums lack are looked up on MusicBrainz. */
  covers_online: boolean;
  /** Whether the photos its artists lack are looked up on Deezer. */
  artist_photos_online: boolean;
}

/** What a library of music does until an administrator says otherwise, the
    same as the server's: nothing asked of anybody online, no file written. */
export const NO_MUSIC_OPTIONS: MusicLibraryOptions = {
  lyrics_online: false,
  tag_writing: false,
  covers_online: false,
  artist_photos_online: false,
};

/** One playlist of songs, as the list of them shows it. */
export interface MusicPlaylist {
  id: string;
  name: string;
  songs: number;
  seconds: number;
  /** The cover of the album of its first song. */
  cover: Picture[];
}

/** One playlist of songs with its songs, in its order. */
export interface MusicPlaylistPage {
  id: string;
  name: string;
  tracks: Song[];
}

/** The tags a song's file carries, as the tag manager edits them. */
export interface EditedTags {
  title: string | null;
  artists: string[];
  album: string | null;
  album_artists: string[];
  track: number | null;
  disc: number | null;
  year: number | null;
  genres: string[];
  compilation: boolean;
}

/** One song's file and what it carries now. */
export interface SongTags {
  song: string;
  file_name: string;
  tags: EditedTags;
}

/** What writing would do to one song. */
export interface PlannedTags {
  song: string;
  file_name: string;
  new_file_name: string | null;
  changed: (keyof EditedTags)[];
  before: EditedTags;
  after: EditedTags;
}

/** What is asked of the tag manager. */
export interface TagsAsked {
  songs: { song: string; tags: EditedTags }[];
  pattern: string | null;
  keep_a_copy: boolean;
}

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
  /** The songs a whole library is played from, in the order of its albums. */
  queue: (library: string, offset: number, signal?: AbortSignal) =>
    get<Page<Song>>(`/api/v1/music/${library}/queue${query({ offset })}`, signal),
  genres: (library: string, signal?: AbortSignal) =>
    get<Genre[]>(`/api/v1/music/${library}/genres`, signal),
  initials: (library: string, of: "albums" | "artists" | "album_artists", signal?: AbortSignal) =>
    get<Initial[]>(`/api/v1/music/${library}/initials${query({ of })}`, signal),
  preferences: (signal?: AbortSignal) =>
    get<MusicPreferences>("/api/v1/music/preferences", signal),
  setPreferences: (chosen: MusicPreferences) =>
    put<MusicPreferences>("/api/v1/music/preferences", chosen),
  favouriteIds: (signal?: AbortSignal) => get<string[]>("/api/v1/music/favourites", signal),
  favourites: (library: string, signal?: AbortSignal) =>
    get<Found>(`/api/v1/music/${library}/favourites`, signal),
  listened: (library: string, order: "lately" | "most", signal?: AbortSignal) =>
    get<Song[]>(`/api/v1/music/${library}/listened${query({ order })}`, signal),
  recordListen: (song: string) => post<{ listened: boolean }>(`/api/v1/music/songs/${song}/listened`),
  lyrics: (song: string, signal?: AbortSignal) =>
    get<SongLyrics | null>(`/api/v1/music/songs/${song}/lyrics`, signal),
  /** Nothing waits on it: it is a line in a journal. */
  tellTheJournalOfLyrics: (said: LyricsSaw) =>
    post<{ written: boolean }>("/api/v1/system/journal/lyrics", said).catch(() => {}),
  /** What LRCLIB holds under an artist and a title, for an administrator. */
  lyricsOffers: (song: string, artist: string, title: string, signal?: AbortSignal) =>
    get<LyricsOffer[]>(`/api/v1/music/songs/${song}/lyrics/offers${query({ artist, title })}`, signal),
  takeLyrics: (song: string, entry: number) => post<null>(`/api/v1/music/songs/${song}/lyrics/online`, { entry }),
  forgetLyrics: (song: string) => remove<null>(`/api/v1/music/songs/${song}/lyrics/online`),
  synchroniseLyrics: (song: string) => post<{ job_id: string }>(`/api/v1/music/songs/${song}/lyrics/sync`, {}),
  unsynchroniseLyrics: (song: string) => remove<null>(`/api/v1/music/songs/${song}/lyrics/sync`),
  /** Nothing for a song whose sound was not read yet. */
  spectrum: async (song: string, signal?: AbortSignal): Promise<SongSpectrum | null> => {
    const answer = await getRaw(`/api/v1/music/songs/${song}/spectrum`, "application/octet-stream", signal);
    if (answer.status === 204) {
      return null;
    }
    const bands = Number(answer.headers.get("x-spectrum-bands"));
    const framesASecond = Number(answer.headers.get("x-spectrum-frames-a-second"));
    const levels = new Uint8Array(await answer.arrayBuffer());
    const whole = bands >= 2 && framesASecond > 0 && levels.length >= bands && levels.length % bands === 0;
    return whole ? { bands, framesASecond, levels } : null;
  },
  libraryOptions: (library: string, signal?: AbortSignal) =>
    get<MusicLibraryOptions>(`/api/v1/music/${library}/options`, signal),
  setLibraryOptions: (library: string, options: MusicLibraryOptions) =>
    put<MusicLibraryOptions>(`/api/v1/music/${library}/options`, options),
  playlists: (signal?: AbortSignal) => get<MusicPlaylist[]>("/api/v1/music/playlists", signal),
  playlist: (id: string, signal?: AbortSignal) =>
    get<MusicPlaylistPage>(`/api/v1/music/playlists/${id}`, signal),
  createPlaylist: (name: string, songs: string[]) =>
    post<{ id: string }>("/api/v1/music/playlists", { name, songs }),
  renamePlaylist: (id: string, name: string) => put<null>(`/api/v1/music/playlists/${id}/name`, { name }),
  deletePlaylist: (id: string) => remove<null>(`/api/v1/music/playlists/${id}`),
  addToPlaylist: (id: string, songs: string[]) =>
    post<null>(`/api/v1/music/playlists/${id}/songs`, { songs }),
  setPlaylistSongs: (id: string, songs: string[]) =>
    put<null>(`/api/v1/music/playlists/${id}/songs`, { songs }),
  albumTags: (album: string, signal?: AbortSignal) =>
    get<SongTags[]>(`/api/v1/music/albums/${album}/tags`, signal),
  previewTags: (asked: TagsAsked) => post<PlannedTags[]>("/api/v1/music/tags/preview", asked),
  writeTags: (asked: TagsAsked) =>
    post<{ written: number; failed: { song: string; reason: string }[] }>("/api/v1/music/tags/write", asked),
  setCover: (album: string, jpeg: Blob, keepACopy: boolean) =>
    put<null>(`/api/v1/music/albums/${album}/cover?keep_a_copy=${keepACopy}`, jpeg),
  search: (words: string, library: string | undefined, signal?: AbortSignal) =>
    get<Found>(`/api/v1/music/search${query({ words, library })}`, signal),
  album: (id: string, signal?: AbortSignal) =>
    get<AlbumPage>(`/api/v1/music/albums/${id}`, signal),
  artistSongs: (id: string) => get<Song[]>(`/api/v1/music/artists/${id}/songs`),
  artist: (id: string, signal?: AbortSignal) =>
    get<ArtistPage>(`/api/v1/music/artists/${id}`, signal),
};
