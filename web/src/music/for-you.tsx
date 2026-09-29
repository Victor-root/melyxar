/*
 * The two tabs of a library of music that are this account's own: the one
 * it opens on, with what arrived lately and what was listened to lately and
 * most, and the one of what it likes.
 *
 * What was listened to is read again each time a listen is counted, and
 * what is liked follows the hearts at once: a song taken off here leaves the
 * list the moment its heart goes out.
 */

import { useEffect, useMemo, useState } from "react";
import { Row, RowHead } from "../components/row";
import { useSettings } from "../settings";
import { music } from "./api";
import type { Album, Found, Song } from "./api";
import { useMusicMarks } from "./marks";
import { useMusic } from "./player/player";
import { MusicFound, foundAny } from "./search";
import { SongList } from "./songs";
import { AlbumTile } from "./tiles";

/** As many albums as the row of the newest on the home page. */
const NEWEST = 20;

export function ForYouTab({ library }: { library: string }) {
  const { t } = useSettings();
  const { listenedAt } = useMusicMarks();
  const player = useMusic();
  const [newest, setNewest] = useState<Album[] | null>(null);
  const [lately, setLately] = useState<Song[]>([]);
  const [most, setMost] = useState<Song[]>([]);

  useEffect(() => {
    const stop = new AbortController();
    music
      .albums(library, "added", true, 0, NEWEST, {}, stop.signal)
      .then((page) => setNewest(page.items))
      .catch(() => {});
    return () => stop.abort();
  }, [library]);

  useEffect(() => {
    const stop = new AbortController();
    music.listened(library, "lately", stop.signal).then(setLately).catch(() => {});
    music.listened(library, "most", stop.signal).then(setMost).catch(() => {});
    return () => stop.abort();
  }, [library, listenedAt]);

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
      <SongList songs={songs} numbered="place" onPlay={onPlay} />
    </section>
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
  const { liked } = useMusicMarks();
  const [found, setFound] = useState<Found | null>(null);

  useEffect(() => {
    const stop = new AbortController();
    music.favourites(library, stop.signal).then(setFound).catch(() => {});
    return () => stop.abort();
  }, [library]);

  const shown = useMemo(() => (found ? stillLiked(found, liked) : null), [found, liked]);
  if (!shown) {
    return null;
  }
  if (!foundAny(shown)) {
    return <p className="notice">{t("music.no_favourite")}</p>;
  }
  return <MusicFound found={shown} />;
}
