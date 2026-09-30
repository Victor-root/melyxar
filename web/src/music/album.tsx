/*
 * The page of one album: its cover, whose it is, and its songs disc by disc.
 */

import { PageBackdrop } from "../components/backdrop";
import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { howMany } from "../readable";
import { useAccount } from "../account";
import { PlayIcon, TagIcon } from "../icons";
import { useLibraryVersion } from "../libraries";
import { useSettings } from "../settings";
import { music } from "./api";
import type { AlbumPage } from "./api";
import { byDisc, minutesOf } from "./discs";
import { Heart } from "./heart";
import { SongMenuButton } from "./song-menu";
import { ShuffleIcon } from "./player/icons";
import { useMusic } from "./player/player";
import { SongList } from "./songs";
import { AlbumCover, namesOf } from "./tiles";

export function MusicAlbumPage() {
  const { t } = useSettings();
  const { id = "" } = useParams();
  const player = useMusic();
  const { account } = useAccount();
  /* Read again whenever its library moves, which is how the album follows
     tags written into its files and a scan filing them anew. */
  const [inLibrary, setInLibrary] = useState<string | null>(null);
  const version = useLibraryVersion(inLibrary);
  const [album, setAlbum] = useState<AlbumPage | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => setAlbum(null), [id]);

  useEffect(() => {
    const stop = new AbortController();
    setFailed(false);
    music
      .album(id, stop.signal)
      .then((read) => {
        setAlbum(read);
        setInLibrary(read.library);
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
      <PageBackdrop />
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
          <div className="music-hero-actions">
            <button
              type="button"
              className="button button-accent"
              disabled={album.tracks.length === 0}
              onClick={() => player.play(album.tracks, 0)}
            >
              <PlayIcon size={18} />
              {t("music.play")}
            </button>
            <button
              type="button"
              className="button"
              disabled={album.tracks.length === 0}
              onClick={() => player.play(album.tracks, Math.floor(Math.random() * album.tracks.length), true)}
            >
              <ShuffleIcon size={18} />
              {t("music.shuffle")}
            </button>
            <Heart id={album.id} size={20} className="music-hero-heart" />
            <SongMenuButton
              songs={album.tracks}
              label={t("music.more_about", { title: album.title })}
              className="music-hero-heart"
            />
            {account?.may_edit_tags && (
              <Link className="button button-quiet" to={`/music/album/${album.id}/tags`}>
                <TagIcon size={16} />
                {t("music.edit_tags")}
              </Link>
            )}
          </div>
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
            onPlay={(index) => player.play(album.tracks, album.tracks.indexOf(disc.songs[index]))}
          />
        </section>
      ))}
    </main>
  );
}
