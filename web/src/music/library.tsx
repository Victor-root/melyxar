/*
 * A library of music: what is new and what was listened to, its albums, its
 * artists, its songs, this account's playlists and what it likes, and its
 * genres, each under a tab of its own, as the servers people come from lay
 * it out.
 *
 * Which tab is open, and how its list is read, is written in the address, so
 * going back to the library finds it the way it was left.
 */

import { Fragment, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { Link, useSearchParams } from "react-router-dom";
import type { Library } from "../api";
import { PageBackdrop } from "../components/backdrop";
import { Picker } from "../components/panel";
import { ArrowRightIcon, CloseIcon } from "../icons";
import { landOn, scrollerOf } from "../landing";
import { useLibraryVersion } from "../libraries";
import { howMany } from "../readable";
import { useSettings } from "../settings";
import { music } from "./api";
import type { AlbumOrder, Genre, Initial, SongOrder } from "./api";
import { FavouritesTab, ForYouTab, PlaylistsTab } from "./for-you";
import { usePaged } from "./paging";
import { useMusic } from "./player/player";
import type { Paged } from "./paging";
import { SongList } from "./songs";
import { UploadButton } from "../components/upload-button";
import { PlayTools } from "./play-tools";
import { TabsBar } from "./tabs-bar";
import { openTab, shownTabs } from "./tabs";
import type { MusicTab } from "./tabs";
import { AlbumTile, ArtistTile, LetterStarts } from "./tiles";


const ALBUM_ORDERS: AlbumOrder[] = ["title", "artist", "year", "added"];
const SONG_ORDERS: SongOrder[] = ["title", "album", "added"];

export function MusicLibraryPage({ library }: { library: Library }) {
  const [params, setParams] = useSearchParams();
  const { preferences } = useMusic();
  const shown = shownTabs(preferences.hidden_tabs);
  const tab = openTab(params.get("tab"), shown);
  const open = (next: MusicTab) => setParams(next === shown[0] ? {} : { tab: next }, { replace: true });

  return (
    <main className="page music-page">
      <PageBackdrop />
      <div className="browse-head">
        <div className="section-head">
          <h1>{library.name}</h1>
        </div>
        <div className="browse-bar">
          <TabsBar tabs={shown} open={tab} onOpen={open} />
          <UploadButton library={library} bare className="browse-piece browse-alone" />
        </div>
      </div>

      {tab === "for_you" && <ForYouTab library={library.id} />}
      {tab === "albums" && <AlbumsTab library={library.id} />}
      {tab === "album_artists" && <ArtistsTab library={library.id} albumArtistsOnly />}
      {tab === "artists" && <ArtistsTab library={library.id} albumArtistsOnly={false} />}
      {tab === "songs" && <SongsTab library={library.id} />}
      {tab === "playlists" && <PlaylistsTab />}
      {tab === "favourites" && <FavouritesTab library={library.id} />}
      {tab === "genres" && <GenresTab library={library.id} />}
    </main>
  );
}

function AlbumsTab({ library }: { library: string }) {
  const { t } = useSettings();
  const version = useLibraryVersion(library);
  const [params, setParams] = useSearchParams();
  const order = ALBUM_ORDERS.find((one) => one === params.get("order")) ?? "title";
  const descending = params.get("descending") === "true";
  const genre = params.get("genre");
  const choose = (name: string, value: string | null) => {
    const next = new URLSearchParams(params);
    if (value === null) {
      next.delete(name);
    } else {
      next.set(name, value);
    }
    setParams(next, { replace: true });
  };

  const albums = usePaged(
    `${library}|${order}|${descending}|${genre ?? ""}`,
    (offset, limit, signal) => music.albums(library, order, descending, offset, limit, { genre }, signal),
    version,
  );
  const letters = useInitials(library, "albums", order === "title" && !descending && !genre);

  return (
    <>
      <div className="browse-bar music-bar">
        <div className="browse-piece">
          <span className="browse-field">
            <span className="browse-label">{t("library.sort")}</span>
            <Picker
              value={order}
              options={ALBUM_ORDERS.map((value) => [value, t(`music.sort.${value}`)] as const)}
              onPick={(value) => choose("order", value === "title" ? null : value)}
              label={t("library.sort")}
            />
            <Direction descending={descending} onFlip={() => choose("descending", descending ? null : "true")} />
          </span>
          {genre && (
            <span className="browse-field browse-field-on">
              <span className="browse-label">{t("music.tab.genres")}</span>
              <span className="music-genre-on">{genre}</span>
              <button
                type="button"
                className="browse-clear"
                onClick={() => choose("genre", null)}
                aria-label={t("library.filter.clear", { name: genre })}
                title={t("library.filter.clear", { name: genre })}
              >
                <CloseIcon size={14} />
              </button>
            </span>
          )}
        </div>
        <PlayTools library={library} />
        {albums.total !== null && (
          <span className="count">{howMany(albums.total, "music.albums_count", t)}</span>
        )}
      </div>
      <Lettered
        paged={albums}
        letters={letters}
        listKey={`${library}|${order}|${descending}|${genre ?? ""}`}
        empty="music.no_album"
      >
        {(starts) => (
          <div className="music-grid">
            {albums.items.map((album, index) => (
              <Fragment key={album.id}>
                {starts?.offset === index && <LetterStarts offset={index} letter={starts.letter} />}
                <AlbumTile album={album} index={index} />
              </Fragment>
            ))}
          </div>
        )}
      </Lettered>
    </>
  );
}

function ArtistsTab({ library, albumArtistsOnly }: { library: string; albumArtistsOnly: boolean }) {
  const { t } = useSettings();
  const version = useLibraryVersion(library);
  const artists = usePaged(
    `${library}|${albumArtistsOnly}`,
    (offset, limit, signal) => music.artists(library, albumArtistsOnly, offset, limit, signal),
    version,
  );
  const letters = useInitials(library, albumArtistsOnly ? "album_artists" : "artists", true);
  return (
    <>
      <div className="browse-bar music-bar">
        <PlayTools library={library} />
        {artists.total !== null && (
          <span className="count">{howMany(artists.total, "music.artists_count", t)}</span>
        )}
      </div>
      <Lettered
        paged={artists}
        letters={letters}
        listKey={`${library}|${albumArtistsOnly}`}
        empty="music.no_artist"
      >
        {(starts) => (
          <div className="music-grid music-grid-artists">
            {artists.items.map((artist, index) => (
              <Fragment key={artist.id}>
                {starts?.offset === index && <LetterStarts offset={index} letter={starts.letter} round />}
                <ArtistTile artist={artist} index={index} />
              </Fragment>
            ))}
          </div>
        )}
      </Lettered>
    </>
  );
}

function SongsTab({ library }: { library: string }) {
  const { t } = useSettings();
  const version = useLibraryVersion(library);
  const [params, setParams] = useSearchParams();
  const order = SONG_ORDERS.find((one) => one === params.get("order")) ?? "title";
  const descending = params.get("descending") === "true";
  const choose = (name: string, value: string | null) => {
    const next = new URLSearchParams(params);
    if (value === null) {
      next.delete(name);
    } else {
      next.set(name, value);
    }
    setParams(next, { replace: true });
  };
  const player = useMusic();
  const songs = usePaged(`${library}|${order}|${descending}`, (offset, limit, signal) =>
    music.songs(library, order, descending, offset, limit, signal),
    version,
  );
  return (
    <>
      <div className="browse-bar music-bar">
        <div className="browse-piece">
          <span className="browse-field">
            <span className="browse-label">{t("library.sort")}</span>
            <Picker
              value={order}
              options={SONG_ORDERS.map((value) => [value, t(`music.sort.${value}`)] as const)}
              onPick={(value) => choose("order", value === "title" ? null : value)}
              label={t("library.sort")}
            />
            <Direction descending={descending} onFlip={() => choose("descending", descending ? null : "true")} />
          </span>
        </div>
        <PlayTools library={library} />
        {songs.total !== null && (
          <span className="count">{howMany(songs.total, "music.songs_count", t)}</span>
        )}
      </div>
      <Lettered paged={songs} letters={null} listKey={`${library}|${order}|${descending}`} empty="music.no_song">
        <SongList songs={songs.items} numbered="place" onPlay={(index) => player.play(songs.items, index)} />
      </Lettered>
    </>
  );
}

function GenresTab({ library }: { library: string }) {
  const { t } = useSettings();
  const version = useLibraryVersion(library);
  const [genres, setGenres] = useState<Genre[] | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    const stop = new AbortController();
    music
      .genres(library, stop.signal)
      .then(setGenres)
      .catch(() => {
        if (!stop.signal.aborted) {
          setFailed(true);
        }
      });
    return () => stop.abort();
  }, [library, version]);

  if (failed) {
    return <p className="notice">{t("error.unreachable")}</p>;
  }
  if (genres === null) {
    return <p className="notice">{t("library.loading")}</p>;
  }
  if (genres.length === 0) {
    return <p className="notice">{t("music.no_genre")}</p>;
  }
  return (
    <div className="music-genres">
      {genres.map((genre) => (
        <Link
          key={genre.name}
          className="music-genre"
          to={`?${new URLSearchParams({ genre: genre.name }).toString()}`}
        >
          <span className="music-genre-name">{genre.name}</span>
          <span className="card-year">{howMany(genre.albums, "music.albums_count", t)}</span>
        </Link>
      ))}
    </div>
  );
}

/** The arrow that turns the order of a list round. */
function Direction({ descending, onFlip }: { descending: boolean; onFlip: () => void }) {
  const { t } = useSettings();
  return (
    <button
      type="button"
      className={`browse-direction${descending ? " browse-direction-down" : ""}`}
      onClick={onFlip}
      aria-pressed={descending}
      aria-label={t("library.descending")}
      title={t(descending ? "library.descending" : "library.ascending")}
    >
      <ArrowRightIcon size={16} />
    </button>
  );
}

/** The letters of a list, when it is read by name. */
function useInitials(
  library: string,
  of: "albums" | "artists" | "album_artists",
  wanted: boolean,
): Initial[] | null {
  const [letters, setLetters] = useState<Initial[] | null>(null);
  const version = useLibraryVersion(library);
  useEffect(() => setLetters(null), [library, of, wanted]);
  useEffect(() => {
    if (!wanted) {
      return;
    }
    const stop = new AbortController();
    music
      .initials(library, of, stop.signal)
      .then(setLetters)
      .catch(() => {
        // No rail, which is what a list the server could not count shows.
      });
    return () => stop.abort();
  }, [library, of, wanted, version]);
  return letters;
}

/** The place in a list a letter begins at, and the letter. */
interface LetterMark {
  offset: number;
  letter: string;
}

/**
 * A list, what it says while it is empty, the letters beside it, and what
 * reads its next page as its end comes near.
 */
function Lettered<T>({
  paged,
  letters,
  listKey,
  empty,
  children,
}: {
  paged: Paged<T>;
  letters: Initial[] | null;
  /** What the list is, so the letter marked is dropped when it is read
   *  another way. */
  listKey: string;
  empty: string;
  /** The list, given where the letter last jumped to begins. */
  children: ReactNode | ((starts: LetterMark | null) => ReactNode);
}) {
  const { t } = useSettings();
  const holder = useRef<HTMLDivElement>(null);
  const end = useRef<HTMLDivElement>(null);
  const { loadMore, more } = paged;

  useEffect(() => {
    const target = end.current;
    if (!target || !more) {
      return;
    }
    const watcher = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          loadMore();
        }
      },
      { rootMargin: "800px" },
    );
    watcher.observe(target);
    return () => watcher.disconnect();
  }, [loadMore, more]);

  /* Where the letter last jumped to begins, marked in the list itself, and
     kept until another letter is chosen or the list is read another way. */
  const [starts, setStarts] = useState<LetterMark | null>(null);
  const [landing, setLanding] = useState<LetterMark | null>(null);
  useEffect(() => setStarts(null), [listKey]);

  const jumpTo = async (letter: Initial) => {
    if (await paged.reach(letter.offset)) {
      const mark = { offset: letter.offset, letter: letter.letter };
      setStarts(mark);
      setLanding(mark);
    }
  };

  const { items } = paged;
  useEffect(() => {
    if (!landing) {
      return;
    }
    const target = holder.current?.querySelector<HTMLElement>(`[data-starts="${landing.offset}"]`);
    // Not drawn yet: the entries that hold it are on their way to the screen,
    // and this runs again when they arrive.
    if (!target) {
      return;
    }
    const box = scrollerOf(target);
    if (!box) {
      setLanding(null);
      return;
    }
    return landOn(target, box, () => setLanding(null));
  }, [landing, items]);

  const shownLetters = letters && letters.length > 1 ? letters : null;
  return (
    <>
      {paged.failed && <p className="notice">{t("error.unreachable")}</p>}
      {!paged.failed && paged.total === 0 && <p className="notice">{t(empty)}</p>}
      <div
        className={`grid-with-letters${shownLetters ? " grid-with-letters-kept" : ""}`}
        ref={holder}
      >
        <div className="music-list">
          {typeof children === "function" ? children(starts) : children}
          <div ref={end} aria-hidden="true" />
        </div>
        {shownLetters && (
          <nav
            className="letters"
            aria-label={t("library.letters")}
            style={{ ["--letters" as string]: shownLetters.length }}
          >
            {shownLetters.map((letter) => (
              <button
                key={letter.letter}
                type="button"
                className="letter"
                onClick={() => void jumpTo(letter)}
                title={String(letter.count)}
              >
                {letter.letter.toUpperCase()}
              </button>
            ))}
          </nav>
        )}
      </div>
      {paged.loading && paged.items.length === 0 && (
        <p className="notice">{t("library.loading")}</p>
      )}
    </>
  );
}
