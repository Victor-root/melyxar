/*
 * The tabs of a library of music that are this account's own: the one it
 * opens on, with what arrived lately and what was listened to lately and
 * most, the one of its playlists, and the one of what it likes.
 *
 * What was listened to is read again each time a listen is counted, and
 * what is liked follows the hearts at once: a song taken off here leaves the
 * list the moment its heart goes out.
 */

import { useEffect, useMemo, useState } from "react";
import { Row, RowHead } from "../components/row";
import { useLibraryVersion } from "../libraries";
import { useSettings } from "../settings";
import { music } from "./api";
import type { Album, Found, MusicPlaylist, Song } from "./api";
import { useMusicMarks } from "./marks";
import { useMusic } from "./player/player";
import { useKeptState } from "./keeping";
import { MusicFound, foundAny } from "./search";
import { SongTiles } from "./song-tiles";
import { AlbumTile, PlaylistTile } from "./tiles";

/** As many albums as the row of the newest on the home page. */
const NEWEST = 20;

export function ForYouTab({ library }: { library: string }) {
  const { t } = useSettings();
  const version = useLibraryVersion(library);
  const { listenedAt } = useMusicMarks();
  const player = useMusic();
  const [newest, setNewest] = useKeptState<Album[] | null>(`for-you|newest|${library}`, null);
  const [lately, setLately] = useKeptState<Song[]>(`for-you|lately|${library}`, []);
  const [most, setMost] = useKeptState<Song[]>(`for-you|most|${library}`, []);

  useEffect(() => {
    const stop = new AbortController();
    music
      .albums(library, "added", true, 0, NEWEST, {}, stop.signal)
      .then((page) => setNewest(page.items))
      .catch(() => {});
    return () => stop.abort();
  }, [library, version, setNewest]);

  useEffect(() => {
    const stop = new AbortController();
    music.listened(library, "lately", stop.signal).then(setLately).catch(() => {});
    music.listened(library, "most", stop.signal).then(setMost).catch(() => {});
    return () => stop.abort();
  }, [library, listenedAt, setLately, setMost]);

  if (newest && newest.length === 0) {
    return <p className="notice">{t("music.no_album")}</p>;
  }
  return (
    <div className="music-for-you">
      {newest && (
        <section className="section">
          <RowHead title={t("music.newest")} />
          <Row>
            {newest.map((album) => (
              <AlbumTile key={album.id} album={album} />
            ))}
          </Row>
        </section>
      )}
      <Listened title={t("music.listened_lately")} songs={lately} onPlay={(index) => player.play(lately, index)} />
      <Listened title={t("music.listened_most")} songs={most} onPlay={(index) => player.play(most, index)} />
    </div>
  );
}

function Listened({ title, songs, onPlay }: { title: string; songs: Song[]; onPlay: (index: number) => void }) {
  if (songs.length === 0) {
    return null;
  }
  return (
    <section className="section">
      <RowHead title={title} />
      <SongTiles songs={songs} onPlay={onPlay} />
    </section>
  );
}

export function PlaylistsTab() {
  const { t } = useSettings();
  const { playlistsAt } = useMusicMarks();
  const [playlists, setPlaylists] = useState<MusicPlaylist[] | null>(null);

  useEffect(() => {
    const stop = new AbortController();
    music.playlists(stop.signal).then(setPlaylists).catch(() => {});
    return () => stop.abort();
  }, [playlistsAt]);

  if (!playlists) {
    return null;
  }
  if (playlists.length === 0) {
    return <p className="notice">{t("music.no_playlist")}</p>;
  }
  return (
    <div className="music-list">
      <div className="music-grid">
        {playlists.map((playlist) => (
          <PlaylistTile key={playlist.id} playlist={playlist} />
        ))}
      </div>
    </div>
  );
}

/** What is liked, as the server last said, less what has been unliked
 *  since. */
export function stillLiked(found: Found, liked: (id: string) => boolean): Found {
  return {
    artists: found.artists.filter((artist) => liked(artist.id)),
    albums: found.albums.filter((album) => liked(album.id)),
    songs: found.songs.filter((song) => liked(song.id)),
  };
}

export function FavouritesTab({ library }: { library: string }) {
  const { t } = useSettings();
  const version = useLibraryVersion(library);
  const { liked } = useMusicMarks();
  const [found, setFound] = useState<Found | null>(null);

  useEffect(() => {
    const stop = new AbortController();
    music.favourites(library, stop.signal).then(setFound).catch(() => {});
    return () => stop.abort();
  }, [library, version]);

  const shown = useMemo(() => (found ? stillLiked(found, liked) : null), [found, liked]);
  if (!shown) {
    return null;
  }
  if (!foundAny(shown)) {
    return <p className="notice">{t("music.no_favourite")}</p>;
  }
  return <MusicFound found={shown} />;
}
