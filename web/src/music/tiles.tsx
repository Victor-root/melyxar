/*
 * An album and an artist, as a grid shows them: a square cover under which
 * the album is named with whose it is, and a round picture over an artist's
 * name.
 */

import { Link } from "react-router-dom";
import { useShownPicture } from "../components/picture";
import { howMany } from "../readable";
import { useSettings } from "../settings";
import type { Album, Artist, Credited } from "./api";

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

export function AlbumTile({ album, index }: { album: Album; index?: number }) {
  const whose = useWhoseAlbum();
  return (
    <Link className="music-tile" to={`/music/album/${album.id}`} data-index={index}>
      <Cover pictures={album.cover} color={album.color} name={album.title} />
      <span className="music-tile-name">{album.title}</span>
      <span className="music-tile-note">
        {[whose(album), album.year].filter(Boolean).join(" · ")}
      </span>
    </Link>
  );
}

export function ArtistTile({ artist, index }: { artist: Artist; index?: number }) {
  const { t } = useSettings();
  return (
    <Link
      className="music-tile music-tile-artist"
      to={`/music/artist/${artist.id}`}
      data-index={index}
    >
      <Cover pictures={artist.picture} color={artist.color} name={artist.name} round />
      <span className="music-tile-name">{artist.name}</span>
      <span className="music-tile-note">
        {artist.albums > 0
          ? howMany(artist.albums, "music.albums_count", t)
          : howMany(artist.songs, "music.songs_count", t)}
      </span>
    </Link>
  );
}

/** The cover of an album, larger, for the head of its own page. */
export function AlbumCover({ album }: { album: Album }) {
  return <Cover pictures={album.cover} color={album.color} name={album.title} />;
}

/** The picture of an artist, larger, for the head of their own page. */
export function ArtistPicture({ artist }: { artist: Artist }) {
  return <Cover pictures={artist.picture} color={artist.color} name={artist.name} round />;
}
