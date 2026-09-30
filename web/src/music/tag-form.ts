/*
 * The pure part of the tag manager's screen: tags as the fields show them,
 * read back from what was typed, and what the album's own fields do to
 * every song of it.
 */

import type { EditedTags } from "./api";

/** Several names in one field, as a person types them. */
export function namesField(names: string[]): string {
  return names.join("; ");
}

/** A field of names read back: split where a person separates them, blanks
 *  left out. */
export function namesIn(field: string): string[] {
  return field
    .split(";")
    .map((name) => name.trim())
    .filter((name) => name !== "");
}

/** A number field read back: nothing for blank, and nothing for what is not
 *  a whole number above nought. */
export function numberOf(field: string): number | null {
  const value = Number(field.trim());
  return field.trim() !== "" && Number.isInteger(value) && value > 0 ? value : null;
}

/** A text field read back: nothing for blank. */
export function textOf(field: string): string | null {
  return field.trim() === "" ? null : field.trim();
}

/** What the album's own fields are, taken from its first song. */
export interface AlbumFields {
  album: string;
  albumArtists: string;
  year: string;
  genres: string;
  compilation: boolean;
}

export function albumFieldsOf(first: EditedTags | undefined): AlbumFields {
  return {
    album: first?.album ?? "",
    albumArtists: namesField(first?.album_artists ?? []),
    year: first?.year === null || first?.year === undefined ? "" : String(first.year),
    genres: namesField(first?.genres ?? []),
    compilation: first?.compilation ?? false,
  };
}

/** One song's tags with the album's fields laid over them: the album is one
 *  album, and each song of it says the same thing about it. */
export function withAlbum(song: EditedTags, album: AlbumFields): EditedTags {
  return {
    ...song,
    album: textOf(album.album),
    album_artists: namesIn(album.albumArtists),
    year: numberOf(album.year),
    genres: namesIn(album.genres),
    compilation: album.compilation,
  };
}
