/*
 * The page of one album: its cover, whose it is, and its songs disc by disc.
 */

import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { howMany } from "../readable";
import { useSettings } from "../settings";
import { music } from "./api";
import type { AlbumPage } from "./api";
import { byDisc, minutesOf } from "./discs";
import { SongList } from "./songs";
import { AlbumCover, namesOf } from "./tiles";

export function MusicAlbumPage() {
  const { t } = useSettings();
  const { id = "" } = useParams();
  const [album, setAlbum] = useState<AlbumPage | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    const stop = new AbortController();
    setAlbum(null);
    setFailed(false);
    music
      .album(id, stop.signal)
      .then(setAlbum)
      .catch(() => {
        if (!stop.signal.aborted) {
          setFailed(true);
        }
      });
    return () => stop.abort();
  }, [id]);

  if (failed) {
    return (
      <main className="page">
        <p className="notice">{t("music.album_gone")}</p>
      </main>
    );
  }
  if (!album) {
    return (
      <main className="page">
        <p className="notice">{t("library.loading")}</p>
      </main>
    );
  }

  const discs = byDisc(album.tracks);
  const whose = album.compilation ? null : namesOf(album.artists);
  return (
    <main className="page music-page">
      <header className="music-hero">
        <div className="music-hero-picture">
          <AlbumCover album={album} />
        </div>
        <div className="music-hero-words">
          <span className="music-hero-kind">{t(album.compilation ? "music.compilation" : "music.album")}</span>
          <h1>{album.title}</h1>
          <p className="music-hero-by">
            {album.compilation
              ? t("music.various_artists")
              : album.artists.map((artist, index) => (
                  <span key={artist.id}>
                    {index > 0 && ", "}
                    <Link to={`/music/artist/${artist.id}`}>{artist.name}</Link>
                  </span>
                ))}
          </p>
          <p className="music-hero-facts">
            {[
              album.year,
              howMany(album.tracks.length, "music.songs_count", t),
              t("music.minutes", { count: minutesOf(album.tracks) }),
            ]
              .filter(Boolean)
              .join(" · ")}
          </p>
        </div>
      </header>

      {discs.map((disc) => (
        <section className="music-disc" key={disc.number ?? "one"}>
          {disc.number !== null && (
            <h2 className="music-disc-name">{t("music.disc", { number: disc.number })}</h2>
          )}
          <SongList
            songs={disc.songs}
            numbered="track"
            showAlbum={false}
            hideArtists={whose ?? undefined}
          />
        </section>
      ))}
    </main>
  );
}
