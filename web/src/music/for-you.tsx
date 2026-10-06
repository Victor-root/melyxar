/*
 * The tabs of a library of music that are this account's own: the one it
 * opens on, with what arrived lately and what was listened to lately and
 * most, the one of its playlists, and the one of what it likes.
 *
 * What was listened to is read again each time a listen is counted, and
 * what is liked follows the hearts at once: a song taken off here leaves the
 * list the moment its heart goes out.
 */

import { useEffect, useMemo, useRef, useState } from "react";
import { Row, RowHead } from "../components/row";
import { useLibraryVersion } from "../libraries";
import { PHONE, useMediaQuery } from "../media-query";
import { useSettings } from "../settings";
import { music } from "./api";
import type { Album, Found, MusicPlaylist, Song } from "./api";
import { useMusicMarks } from "./marks";
import { useMusic } from "./player/player";
import { useKeptState } from "./keeping";
import { useRoomBelow } from "./room-below";
import { MusicFound, foundAny } from "./search";
import { SongTiles } from "./song-tiles";
import { SongList } from "./songs";
import { AlbumTile, PlaylistTile } from "./tiles";

/** As many albums as the row of the newest on the home page. */
const NEWEST = 20;

/** The width from which the two listened lists stand side by side, each
 *  scrolling in its own box. */
const SIDE_BY_SIDE = "(min-width: 1000px)";

/** How many songs each of the two listened lists shows on a phone, where
 *  they are one list to switch from the one to the other. */
const LISTED_ON_PHONE = 8;

/** The least a list is let shrink to, on a screen too short to give it more. */
const LEAST_LIST = 280;

export function ForYouTab({ library }: { library: string }) {
  const { t } = useSettings();
  const version = useLibraryVersion(library);
  const { listenedAt } = useMusicMarks();
  const player = useMusic();
  const wide = useMediaQuery(SIDE_BY_SIDE);
  const phone = useMediaQuery(PHONE);
  const [newest, setNewest] = useKeptState<Album[] | null>(`for-you|newest|${library}`, null);
  const [lately, setLately] = useKeptState<Song[]>(`for-you|lately|${library}`, []);
  const [most, setMost] = useKeptState<Song[]>(`for-you|most|${library}`, []);

  useEffect(() => {
    const stop = new AbortController();
    music
      .albums(library, "added", false, 0, NEWEST, {}, stop.signal)
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
      {phone ? (
        <ListenedSwitch
          lists={[
            { title: t("music.listened_lately"), songs: lately },
            { title: t("music.listened_most"), songs: most },
          ]}
          onPlay={(songs, index) => player.play(songs, index)}
        />
      ) : wide ? (
        <ListenedColumns>
          <ListenedList title={t("music.listened_lately")} songs={lately} onPlay={(index) => player.play(lately, index)} />
          <ListenedList title={t("music.listened_most")} songs={most} onPlay={(index) => player.play(most, index)} />
        </ListenedColumns>
      ) : (
        <>
          <Listened title={t("music.listened_lately")} songs={lately} onPlay={(index) => player.play(lately, index)} />
          <Listened title={t("music.listened_most")} songs={most} onPlay={(index) => player.play(most, index)} />
        </>
      )}
    </div>
  );
}

/**
 * The two listened lists on a phone: one list the whole width of the screen,
 * the other beside it, a swipe or a press on its name away. The one in view
 * is the one whose name is lit.
 */
function ListenedSwitch({
  lists,
  onPlay,
}: {
  lists: { title: string; songs: Song[] }[];
  onPlay: (songs: Song[], index: number) => void;
}) {
  const shown = lists.filter((list) => list.songs.length > 0);
  const track = useRef<HTMLDivElement>(null);
  const pages = useRef<(HTMLDivElement | null)[]>([]);
  const [page, setPage] = useState(0);

  useEffect(() => {
    const element = track.current;
    if (!element) {
      return;
    }
    const watcher = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            setPage(pages.current.indexOf(entry.target as HTMLDivElement));
          }
        }
      },
      { root: element, threshold: 0.6 },
    );
    for (const one of pages.current) {
      if (one) {
        watcher.observe(one);
      }
    }
    return () => watcher.disconnect();
  }, [shown.length]);

  if (shown.length === 0) {
    return null;
  }
  return (
    <section className="section">
      <div className="section-head">
        <h2 className="music-listened-titles">
          {shown.map((list, at) => (
            <button
              key={list.title}
              type="button"
              className="music-listened-title"
              data-on={at === page ? "yes" : "no"}
              onClick={() => pages.current[at]?.scrollIntoView({ behavior: "smooth", inline: "start", block: "nearest" })}
            >
              {list.title}
            </button>
          ))}
        </h2>
      </div>
      <div className="music-listened-pages" ref={track}>
        {shown.map((list, at) => (
          <div
            key={list.title}
            className="music-listened-page"
            ref={(element) => {
              pages.current[at] = element;
            }}
          >
            <SongList
              songs={list.songs.slice(0, LISTED_ON_PHONE)}
              numbered="place"
              showAlbum={false}
              menuOnly
              onPlay={(index) => onPlay(list.songs, index)}
            />
          </div>
        ))}
      </div>
    </section>
  );
}

/** The two lists side by side, as tall as the screen has room for under the
 *  rest of the page. */
function ListenedColumns({ children }: { children: React.ReactNode }) {
  const box = useRef<HTMLDivElement>(null);
  useRoomBelow(box, LEAST_LIST);
  return (
    <div ref={box} className="music-listened">
      {children}
    </div>
  );
}

function ListenedList({ title, songs, onPlay }: { title: string; songs: Song[]; onPlay: (index: number) => void }) {
  if (songs.length === 0) {
    return null;
  }
  return (
    <section className="music-listened-column">
      <RowHead title={title} />
      <div className="music-listened-list">
        <SongList songs={songs} numbered="place" showAlbum={false} menuOnly onPlay={onPlay} />
      </div>
    </section>
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
