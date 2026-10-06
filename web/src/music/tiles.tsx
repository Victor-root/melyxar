/*
 * An album and an artist, as a grid shows them: the card of the rest of the
 * interface over a square cover under which the album is named with whose it
 * is, and over a round picture for an artist.
 */

import { memo, useContext, useState } from "react";
import { Link } from "react-router-dom";
import { PicturesAhead } from "../components/card";
import { useShownPicture } from "../components/picture";
import { ChevronRightIcon, HeartIcon, PlayIcon } from "../icons";
import { PauseIcon } from "../player/icons";
import { howMany } from "../readable";
import { useSettings } from "../settings";
import { music } from "./api";
import type { Album, Artist, Credited, MusicPlaylist, Song } from "./api";
import { useMusicMarks } from "./marks";
import { useMusicControls, useMusicNow } from "./player/player";

/** Whose album it is, as a line under its title. */
export function useWhoseAlbum(): (album: Album) => string {
  const { t } = useSettings();
  return (album) =>
    album.compilation ? t("music.various_artists") : namesOf(album.artists);
}

/** Several artists as one line. */
export function namesOf(artists: Credited[]): string {
  return artists.map((artist) => artist.name).join(", ");
}

/**
 * The picture of an album or an artist, or its colour and its first letter
 * while there is none: a grid never shows a hole.
 */
function Cover({
  pictures,
  color,
  name,
  round = false,
}: {
  pictures: Album["cover"];
  color: string | null;
  name: string;
  round?: boolean;
}) {
  const { picture, itDidNotLoad } = useShownPicture(pictures);
  return (
    <div
      className={`music-cover${round ? " music-cover-round" : ""}`}
      style={color ? { ["--cover-color" as string]: color } : undefined}
    >
      {picture ? (
        <img
          src={picture.src}
          srcSet={picture.srcSet || undefined}
          sizes="(max-width: 600px) 45vw, 220px"
          alt=""
          loading="lazy"
          decoding="async"
          onError={itDidNotLoad}
        />
      ) : (
        <span className="music-cover-letter" aria-hidden="true">
          {name.trim().charAt(0).toUpperCase()}
        </span>
      )}
    </div>
  );
}

/**
 * One tile of a grid or a row: the card of the rest of the interface, with
 * its play button in the middle and its heart in the corner under the
 * pointer, over a square cover or a round picture. What it plays is asked of
 * the server at the press, since a grid holds thousands of them.
 */
function Tile({
  to,
  index,
  pictures,
  color,
  name,
  note,
  round = false,
  liking,
  songs,
  owns,
}: {
  to: string;
  index?: number;
  pictures: Album["cover"];
  color: string | null;
  name: string;
  note: string;
  round?: boolean;
  /** What the heart likes, when the tile has one. */
  liking?: string;
  /** The songs a press of play starts, in the order they play. */
  songs: () => Promise<Song[]>;
  /** Whether a song playing or paused is one of this tile's, so its button
   *  pauses and resumes it rather than starting it over. */
  owns?: (song: Song) => boolean;
}) {
  const { t } = useSettings();
  const player = useMusicControls();
  /* Whether a song of this tile's is playing or paused, and nothing more, so a
     song starting draws again only the tiles it concerns. */
  const state = useMusicNow((song, playing) =>
    owns === undefined || song === null || !owns(song) ? "other" : playing ? "playing" : "paused",
  );
  const marks = useMusicMarks();
  const { picture, itDidNotLoad } = useShownPicture(pictures);
  const ahead = useContext(PicturesAhead).now;
  /* Made the first time the tile is reached, like the card of a film: a
     grid of thousands does not hold thousands of hidden buttons. */
  const [reached, setReached] = useState(false);
  const liked = liking !== undefined && marks.liked(liking);
  const stop = (doing: () => void) => (event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    doing();
  };
  const ours = state !== "other";
  const going = state === "playing";
  const play = () =>
    ours
      ? player.toggle()
      : songs()
          .then((found) => found.length > 0 && player.play(found, 0))
          .catch(() => {});
  return (
    <article
      className={`card card-square${round ? " card-round" : ""}`}
      data-index={index}
      style={{ ["--card-color" as string]: color ?? "var(--surface-raised)" }}
      onPointerEnter={() => setReached(true)}
      onFocus={() => setReached(true)}
    >
      <div className="card-picture">
        {picture ? (
          <img
            src={picture.src}
            srcSet={picture.srcSet || undefined}
            sizes="(max-width: 700px) 40vw, 186px"
            alt=""
            loading={ahead ? "eager" : "lazy"}
            decoding="async"
            draggable={false}
            onError={itDidNotLoad}
          />
        ) : (
          <span className="card-initial" aria-hidden="true">
            {name.slice(0, 1)}
          </span>
        )}
        <Link className="card-open" to={to} title={name} draggable={false}>
          <span className="visually-hidden">{name}</span>
        </Link>
        {reached && (
          <div className="card-hover">
            <button
              type="button"
              className="card-play"
              aria-label={t(going ? "music.pause" : "music.play")}
              title={t(going ? "music.pause" : "music.play")}
              onClick={stop(play)}
            >
              {going ? <PauseIcon size={32} /> : <PlayIcon size={32} />}
            </button>
            {liking !== undefined && (
              <div className="card-corner">
                <button
                  type="button"
                  className={`card-mark${liked ? " card-mark-on" : ""}`}
                  aria-pressed={liked}
                  aria-label={t(liked ? "card.unfavourite" : "card.favourite")}
                  title={t(liked ? "card.unfavourite" : "card.favourite")}
                  onClick={stop(() => marks.setLiked(liking, !liked))}
                >
                  <HeartIcon size={17} filled={liked} />
                </button>
              </div>
            )}
          </div>
        )}
      </div>
      <span className="card-line">
        <span className="card-title">{name}</span>
      </span>
      <span className="card-year">{note}</span>
    </article>
  );
}

/* Drawn again only when its album is another: a grid of hundreds is not drawn
   again whole each time the page around it is, as when the player opens. */
export const AlbumTile = memo(function AlbumTile({ album, index }: { album: Album; index?: number }) {
  const whose = useWhoseAlbum();
  return (
    <Tile
      to={`/music/album/${album.id}`}
      index={index}
      pictures={album.cover}
      color={album.color}
      name={album.title}
      note={[whose(album), album.year].filter(Boolean).join(" · ")}
      liking={album.id}
      songs={() => music.album(album.id).then((page) => page.tracks)}
      owns={(song) => song.album?.id === album.id}
    />
  );
});

export const ArtistTile = memo(function ArtistTile({ artist, index }: { artist: Artist; index?: number }) {
  const { t } = useSettings();
  return (
    <Tile
      to={`/music/artist/${artist.id}`}
      index={index}
      pictures={artist.picture}
      color={artist.color}
      name={artist.name}
      note={
        artist.albums > 0
          ? howMany(artist.albums, "music.albums_count", t)
          : howMany(artist.songs, "music.songs_count", t)
      }
      round
      liking={artist.id}
      songs={() => music.artistSongs(artist.id)}
      owns={(song) => song.artists.some((credited) => credited.id === artist.id)}
    />
  );
});

/** A playlist of songs, wearing the cover of its first song's album. */
export function PlaylistTile({ playlist }: { playlist: MusicPlaylist }) {
  const { t } = useSettings();
  return (
    <Tile
      to={`/music/playlist/${playlist.id}`}
      pictures={playlist.cover}
      color={null}
      name={playlist.name}
      note={howMany(playlist.songs, "music.songs_count", t)}
      songs={() => music.playlist(playlist.id).then((page) => page.tracks)}
    />
  );
}

/** The cover of a playlist, larger, for the head of its own page. */
export function PlaylistCover({ playlist }: { playlist: { name: string; cover: MusicPlaylist["cover"] } }) {
  return <Cover pictures={playlist.cover} color={null} name={playlist.name} />;
}

/** The cover of an album, larger, for the head of its own page. */
export function AlbumCover({ album }: { album: Album }) {
  return <Cover pictures={album.cover} color={album.color} name={album.title} />;
}

/** The picture of an artist, larger, for the head of their own page. */
export function ArtistPicture({ artist }: { artist: Artist }) {
  return <Cover pictures={artist.picture} color={artist.color} name={artist.name} round />;
}

/**
 * Where the letter just chosen begins, standing in the grid as one more tile
 * before the first of its entries, the way it does in the grids of the films:
 * the row it opens may still end the letter before.
 */
export function LetterStarts({ offset, letter, round }: { offset: number; letter: string; round?: boolean }) {
  return (
    <div
      className={`letter-starts letter-starts-${round ? "round" : "square"}`}
      data-starts={offset}
      aria-hidden="true"
    >
      <span>{letter.toUpperCase()}</span>
      <ChevronRightIcon size={40} />
    </div>
  );
}
