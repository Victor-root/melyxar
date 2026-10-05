/*
 * The page of one artist: their picture, every song they play on played at
 * a press, the albums that are theirs, and the albums of others they play
 * on.
 */

import { PageBackdrop } from "../components/backdrop";
import { useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import { PlayIcon } from "../icons";
import { useLibraryVersion } from "../libraries";
import { howMany } from "../readable";
import { useSettings } from "../settings";
import { music } from "./api";
import type { Album, ArtistPage } from "./api";
import { Heart } from "./heart";
import { useKeptState } from "./keeping";
import { ShuffleIcon } from "./player/icons";
import { useMusic } from "./player/player";
import { AlbumTile, ArtistPicture } from "./tiles";
import { useTabPage } from "../tab-page";

export function MusicArtistPage() {
  const { t } = useSettings();
  const { id = "" } = useParams();
  const [artist, setArtist] = useKeptState<ArtistPage | null>(`artist|${id}`, null);
  useTabPage(artist?.name);
  const [failed, setFailed] = useState(false);
  /* Read again whenever its library moves: an album filed, a cover found. */
  const version = useLibraryVersion(artist?.library);
  const player = useMusic();
  /* Asked for only when pressed: their songs are the whole of their work,
     and most visits to an artist never play all of it. */
  const [starting, setStarting] = useState(false);
  const playAll = (shuffle: boolean) => {
    setStarting(true);
    music
      .artistSongs(id)
      .then((songs) => player.play(songs, shuffle ? Math.floor(Math.random() * songs.length) : 0, shuffle))
      .catch(() => {})
      .finally(() => setStarting(false));
  };

  useEffect(() => {
    const stop = new AbortController();
    setFailed(false);
    music
      .artist(id, stop.signal)
      .then((read) => {
        setArtist(read);
      })
      .catch(() => {
        if (!stop.signal.aborted) {
          setFailed(true);
        }
      });
    return () => stop.abort();
  }, [id, version]);

  if (failed) {
    return (
      <main className="page">
        <p className="notice">{t("music.artist_gone")}</p>
      </main>
    );
  }
  if (!artist) {
    return (
      <main className="page">
        <p className="notice">{t("library.loading")}</p>
      </main>
    );
  }

  return (
    <main className="page music-page">
      <PageBackdrop />
      <header className="music-hero music-hero-artist">
        <div className="music-hero-picture">
          <ArtistPicture artist={artist} />
        </div>
        <div className="music-hero-words">
          <span className="music-hero-kind">{t("music.artist")}</span>
          <h1>{artist.name}</h1>
          <p className="music-hero-facts">
            {[
              artist.albums > 0 && howMany(artist.albums, "music.albums_count", t),
              howMany(artist.songs, "music.songs_count", t),
            ]
              .filter(Boolean)
              .join(" · ")}
          </p>
          <div className="music-hero-actions">
            <button
              type="button"
              className="button button-accent"
              disabled={artist.songs === 0 || starting}
              onClick={() => playAll(false)}
            >
              <PlayIcon size={18} />
              {t("music.play_all")}
            </button>
            <button
              type="button"
              className="button"
              disabled={artist.songs === 0 || starting}
              onClick={() => playAll(true)}
            >
              <ShuffleIcon size={18} />
              {t("music.shuffle")}
            </button>
            <Heart id={artist.id} size={20} className="music-hero-heart" />
          </div>
        </div>
      </header>

      <Albums name={t("music.their_albums")} albums={artist.their_albums} />
      <Albums name={t("music.appears_on")} albums={artist.appears_on} />
    </main>
  );
}

function Albums({ name, albums }: { name: string; albums: Album[] }) {
  if (albums.length === 0) {
    return null;
  }
  return (
    <section className="music-section">
      <h2>{name}</h2>
      <div className="music-grid">
        {albums.map((album) => (
          <AlbumTile key={album.id} album={album} />
        ))}
      </div>
    </section>
  );
}
