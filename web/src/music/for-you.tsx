/*
 * The tabs of a library of music that are this account's own: the one it
 * opens on, with what arrived lately and what was listened to lately and
 * most, the one of its playlists, and the one of what it likes. On a phone
 * the three of the first are pages side by side, a swipe apart.
 *
 * What was listened to is read again each time a listen is counted, and
 * what is liked follows the hearts at once: a song taken off here leaves the
 * list the moment its heart goes out.
 */

import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Row, RowHead } from "../components/row";
import { ScrollBar } from "../components/scrollbar";
import { FlameIcon, HistoryIcon, NewAlbumIcon } from "../icons";
import { useLibraryVersion } from "../libraries";
import { PHONE, useMediaQuery } from "../media-query";
import { useSettings } from "../settings";
import { music } from "./api";
import type { Album, Found, MusicPlaylist, Song } from "./api";
import { useMusicMarks } from "./marks";
import { useMusicControls } from "./player/player";
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

/** The least a list is let shrink to, on a screen too short to give it more. */
const LEAST_LIST = 280;

export function ForYouTab({ library }: { library: string }) {
  const { t } = useSettings();
  const version = useLibraryVersion(library);
  const { listenedAt } = useMusicMarks();
  const player = useMusicControls();
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
  if (phone) {
    const songsOf = (songs: Song[]) => ({
      filled: songs.length > 0,
      content: <SongList songs={songs} numbered="place" showAlbum={false} menuOnly onPlay={(index) => player.play(songs, index)} />,
    });
    return (
      <div className="music-for-you">
        {/* What was listened to first, under the thumb, then what arrived. */}
        <ForYouPages
          pages={[
            { title: t("music.listened_lately"), mark: <HistoryIcon size={20} />, ...songsOf(lately) },
            { title: t("music.listened_most"), mark: <FlameIcon size={20} />, ...songsOf(most) },
            {
              title: t("music.newest"),
              mark: <NewAlbumIcon size={20} />,
              filled: (newest?.length ?? 0) > 0,
              content: (
                <div className="music-grid">
                  {newest?.map((album) => (
                    <AlbumTile key={album.id} album={album} />
                  ))}
                </div>
              ),
            },
          ]}
        />
      </div>
    );
  }
  return (
    <div className="music-for-you">
      {newest && (
        <section className="section">
          <RowHead mark={<NewAlbumIcon size={20} />} title={t("music.newest")} />
          <Row>
            {newest.map((album) => (
              <AlbumTile key={album.id} album={album} />
            ))}
          </Row>
        </section>
      )}
      {wide ? (
        <ListenedColumns>
          <ListenedList mark={<HistoryIcon size={20} />} title={t("music.listened_lately")} songs={lately} onPlay={(index) => player.play(lately, index)} />
          <ListenedList mark={<FlameIcon size={20} />} title={t("music.listened_most")} songs={most} onPlay={(index) => player.play(most, index)} />
        </ListenedColumns>
      ) : (
        <>
          <Listened mark={<HistoryIcon size={20} />} title={t("music.listened_lately")} songs={lately} onPlay={(index) => player.play(lately, index)} />
          <Listened mark={<FlameIcon size={20} />} title={t("music.listened_most")} songs={most} onPlay={(index) => player.play(most, index)} />
        </>
      )}
    </div>
  );
}

/** One page of those of a phone: its name, its mark, and what it holds. */
interface ForYouPage {
  title: string;
  mark: React.ReactNode;
  filled: boolean;
  content: React.ReactNode;
}

/**
 * The pages of a phone: one the whole width of the screen, the others beside
 * it, a swipe or a press on their mark away. The one in view is the one whose
 * mark is lit. Each scrolls in its own box, as tall as the screen allows,
 * which leaves the page itself nothing to scroll.
 */
function ForYouPages({ pages }: { pages: ForYouPage[] }) {
  const shown = pages.filter((one) => one.filled);
  return shown.length === 0 ? null : <PagesTrack shown={shown} />;
}

function PagesTrack({ shown }: { shown: ForYouPage[] }) {
  /* Sideways only: bringing a page into view scrolled the whole page along. A
     page is a width of the row plus the gap between two, which is its padding. */
  const track = useRef<HTMLDivElement>(null);
  const slots = useRef<(HTMLDivElement | null)[]>([]);
  /* Kept, so that coming back from an album opened from the third page finds
     the third page, put back before it is first drawn. */
  const [kept, setPage] = useKeptState("for-you|page", 0);
  const page = Math.min(kept, shown.length - 1);
  useRoomBelow(track, LEAST_LIST, true);
  useLayoutEffect(() => {
    const element = track.current;
    if (element) {
      element.scrollLeft = page * element.clientWidth;
    }
    // Only as it is first drawn: after that the row is where the hand left it.
  }, []);

  useEffect(() => {
    const element = track.current;
    if (!element) {
      return;
    }
    const watcher = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            setPage(slots.current.indexOf(entry.target as HTMLDivElement));
          }
        }
      },
      { root: element, threshold: 0.6 },
    );
    for (const one of slots.current) {
      if (one) {
        watcher.observe(one);
      }
    }
    return () => watcher.disconnect();
  }, [shown.length]);

  return (
    <section className="section">
      <div className="section-head">
        <div className="music-listened-switch">
          {shown.map((one, at) => (
            <button
              key={one.title}
              type="button"
              className="music-listened-title"
              data-on={at === page ? "yes" : "no"}
              aria-label={one.title}
              title={one.title}
              onClick={() => track.current?.scrollTo({ left: at * track.current.clientWidth, behavior: "smooth" })}
            >
              {one.mark}
            </button>
          ))}
        </div>
        <h2>{shown[page].title}</h2>
      </div>
      <div className="music-listened-pages" ref={track}>
        {shown.map((one, at) => (
          <PageSlot
            key={one.title}
            hold={(element) => {
              slots.current[at] = element;
            }}
          >
            {one.content}
          </PageSlot>
        ))}
      </div>
    </section>
  );
}

/** One page of the row, scrolling in its own box. */
function PageSlot({ hold, children }: { hold: (element: HTMLDivElement | null) => void; children: React.ReactNode }) {
  const page = useRef<HTMLDivElement>(null);
  return (
    <div className="music-listened-slot" ref={hold}>
      <div className="music-listened-page" ref={page}>
        {children}
      </div>
      <ScrollBar holder={page} inside />
    </div>
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

function ListenedList({
  mark,
  title,
  songs,
  onPlay,
}: {
  mark: React.ReactNode;
  title: string;
  songs: Song[];
  onPlay: (index: number) => void;
}) {
  if (songs.length === 0) {
    return null;
  }
  return (
    <section className="music-listened-column">
      <RowHead mark={mark} title={title} />
      <div className="music-listened-list">
        <SongList songs={songs} numbered="place" showAlbum={false} menuOnly onPlay={onPlay} />
      </div>
    </section>
  );
}

function Listened({
  mark,
  title,
  songs,
  onPlay,
}: {
  mark: React.ReactNode;
  title: string;
  songs: Song[];
  onPlay: (index: number) => void;
}) {
  if (songs.length === 0) {
    return null;
  }
  return (
    <section className="section">
      <RowHead mark={mark} title={title} />
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
