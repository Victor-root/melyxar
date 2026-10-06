/*
 * What a search found in the music: the artists, the albums and the songs
 * whose name holds the words, a row or a list of each.
 *
 * Asked of music's own route and drawn with music's own tiles, beside the
 * grid of films a search already is, or on its own when the search is
 * narrowed to music.
 */

import { useEffect, useState } from "react";
import type { Library, Picture } from "../api";
import { Row, RowHead } from "../components/row";
import { useSettings } from "../settings";
import { music } from "./api";
import type { Found } from "./api";
import { useMusicControls } from "./player/player";
import { SongList } from "./songs";
import { AlbumTile, ArtistTile, namesOf } from "./tiles";

/** Whether a search in this scope looks in the music, and in which library
 *  of it: every one when it is not narrowed, none when it is narrowed to
 *  something else. */
export function musicScopeOf(
  scope: string,
  libraries: Library[],
): { looks: boolean; library?: string; only: boolean } {
  if (scope === "") {
    return { looks: libraries.some((library) => library.kind === "music"), only: false };
  }
  if (scope === "kind:music") {
    return { looks: true, only: true };
  }
  if (scope.startsWith("library:")) {
    const id = scope.slice("library:".length);
    const music = libraries.some((library) => library.id === id && library.kind === "music");
    return music ? { looks: true, library: id, only: true } : { looks: false, only: false };
  }
  return { looks: false, only: false };
}

/** One line of the few results the search field offers on its own. */
export interface QuickLine {
  key: string;
  to: string;
  title: string;
  note: string | null;
  pictures: Picture[];
  color: string | null;
  /** Square for a cover, round for an artist; a poster otherwise. */
  shape?: "square" | "round";
}

/** What the music found, as a few lines of the search field: artists
 *  first, then albums, then songs, which lead to their album. */
export function quickLinesOf(found: Found | null, room: number): QuickLine[] {
  if (!found) {
    return [];
  }
  return [
    ...found.artists.map((artist) => ({
      key: `artist:${artist.id}`,
      to: `/music/artist/${artist.id}`,
      title: artist.name,
      note: null,
      pictures: artist.picture,
      color: artist.color,
      shape: "round" as const,
    })),
    ...found.albums.map((album) => ({
      key: `album:${album.id}`,
      to: `/music/album/${album.id}`,
      title: album.title,
      note: namesOf(album.artists) || null,
      pictures: album.cover,
      color: album.color,
      shape: "square" as const,
    })),
    ...found.songs
      .filter((song) => song.album !== null)
      .map((song) => ({
        key: `song:${song.id}`,
        to: `/music/album/${song.album!.id}`,
        title: song.title,
        note: namesOf(song.artists) || null,
        pictures: song.cover,
        color: null,
        shape: "square" as const,
      })),
  ].slice(0, room);
}

/** Whether anything at all was found. */
export function foundAny(found: Found | null): boolean {
  return !!found && found.albums.length + found.artists.length + found.songs.length > 0;
}

/** What these words find in the music, read again whenever they change. */
export function useMusicFound(words: string, looks: boolean, library?: string): Found | null {
  const [found, setFound] = useState<Found | null>(null);
  useEffect(() => {
    if (!looks || !words.trim()) {
      setFound(null);
      return;
    }
    const stop = new AbortController();
    music
      .search(words.trim(), library, stop.signal)
      .then(setFound)
      .catch(() => {});
    return () => stop.abort();
  }, [words, looks, library]);
  return found;
}

export function MusicFound({ found }: { found: Found | null }) {
  const { t } = useSettings();
  const player = useMusicControls();
  if (!found || !foundAny(found)) {
    return null;
  }
  return (
    <div className="music-found">
      {found.artists.length > 0 && (
        <section className="section">
          <RowHead title={t("music.tab.artists")} />
          <Row>
            {found.artists.map((artist) => (
              <ArtistTile key={artist.id} artist={artist} />
            ))}
          </Row>
        </section>
      )}
      {found.albums.length > 0 && (
        <section className="section">
          <RowHead title={t("music.tab.albums")} />
          <Row>
            {found.albums.map((album) => (
              <AlbumTile key={album.id} album={album} />
            ))}
          </Row>
        </section>
      )}
      {found.songs.length > 0 && (
        <section className="section">
          <RowHead title={t("music.tab.songs")} />
          <SongList
            songs={found.songs}
            numbered="place"
            onPlay={(index) => player.play(found.songs, index)}
          />
        </section>
      )}
    </div>
  );
}
