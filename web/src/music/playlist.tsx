/*
 * The page of one playlist of songs: played or shuffled at a press, its
 * songs put in order by dragging them and taken out from their menu, the
 * playlist renamed or deleted.
 *
 * What is moved or taken out shows at once and goes back as it was if the
 * server refuses; every other screen showing the playlist reads it again
 * once the server agrees.
 */

import { PageBackdrop } from "../components/backdrop";
import { useEffect, useRef, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { refusalOf } from "../asking";
import { Modal } from "../components/modal";
import { Sortable } from "../components/sortable";
import { refusalKey } from "../i18n";
import { DeleteIcon, EditIcon, PlayIcon } from "../icons";
import { asClock } from "../clock";
import { howMany } from "../readable";
import { useSettings } from "../settings";
import { music } from "./api";
import type { MusicPlaylistPage, Song } from "./api";
import { minutesOf } from "./discs";
import { Heart } from "./heart";
import { useMusicMarks } from "./marks";
import { ShuffleIcon } from "./player/icons";
import { useMusic } from "./player/player";
import { SongMenuButton } from "./song-menu";
import { PlaylistCover, namesOf } from "./tiles";
import { useTabPage } from "../tab-page";

/** A song of the playlist, with a name of its own on the page: the same song
 *  may be in it twice. */
interface Line {
  key: string;
  song: Song;
}

export function MusicPlaylistPage() {
  const { t } = useSettings();
  const { id = "" } = useParams();
  const navigate = useNavigate();
  const player = useMusic();
  const { playlistsAt, playlistsHaveMoved } = useMusicMarks();
  const [playlist, setPlaylist] = useState<MusicPlaylistPage | null>(null);
  useTabPage(playlist?.name);
  const [lines, setLines] = useState<Line[]>([]);
  const [failed, setFailed] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);
  const named = useRef(0);

  useEffect(() => {
    const stop = new AbortController();
    setFailed(false);
    music
      .playlist(id, stop.signal)
      .then((read) => {
        setPlaylist(read);
        setLines(
          read.tracks.map((song) => {
            named.current += 1;
            return { key: String(named.current), song };
          }),
        );
      })
      .catch(() => {
        if (!stop.signal.aborted) {
          setFailed(true);
        }
      });
    return () => stop.abort();
  }, [id, playlistsAt]);

  if (failed) {
    return (
      <main className="page">
        <p className="notice">{t("error.not_found")}</p>
      </main>
    );
  }
  if (!playlist) {
    return <main className="page" aria-busy="true" />;
  }

  const songs = lines.map((line) => line.song);
  const change = async (next: Line[]) => {
    const before = lines;
    setLines(next);
    setRefused(null);
    try {
      await music.setPlaylistSongs(
        id,
        next.map((line) => line.song.id),
      );
      playlistsHaveMoved();
    } catch (error) {
      setLines(before);
      setRefused(refusalOf(error));
    }
  };
  const attempt = async (doing: () => Promise<unknown>) => {
    setRefused(null);
    try {
      await doing();
      return true;
    } catch (error) {
      setRefused(refusalOf(error));
      return false;
    }
  };

  return (
    <main className="page music-page">
      <PageBackdrop />
      <header className="music-hero">
        <div className="music-hero-picture">
          <PlaylistCover playlist={{ name: playlist.name, cover: songs[0]?.cover ?? [] }} />
        </div>
        <div className="music-hero-words">
          <span className="music-hero-kind">{t("music.playlist")}</span>
          {renaming === null ? (
            <h1>{playlist.name}</h1>
          ) : (
            <form
              className="lists-new list-rename"
              onSubmit={(event) => {
                event.preventDefault();
                void attempt(async () => {
                  await music.renamePlaylist(id, renaming);
                  setPlaylist({ ...playlist, name: renaming.trim() });
                  playlistsHaveMoved();
                }).then((done) => done && setRenaming(null));
              }}
            >
              <input
                type="text"
                className="field-line"
                aria-label={t("lists.name")}
                autoFocus
                maxLength={80}
                value={renaming}
                onChange={(event) => setRenaming(event.target.value)}
              />
              <button type="submit" className="button button-small button-accent" disabled={renaming.trim() === ""}>
                {t("lists.keep")}
              </button>
              <button type="button" className="button button-small button-quiet" onClick={() => setRenaming(null)}>
                {t("lists.cancel")}
              </button>
            </form>
          )}
          <p className="music-hero-facts">
            {[howMany(songs.length, "music.songs_count", t), t("music.minutes", { count: minutesOf(songs) })].join(" · ")}
          </p>
          <div className="music-hero-actions">
            <button
              type="button"
              className="button button-accent"
              disabled={songs.length === 0}
              onClick={() => player.play(songs, 0)}
            >
              <PlayIcon size={18} />
              {t("music.play")}
            </button>
            <button
              type="button"
              className="button"
              disabled={songs.length === 0}
              onClick={() => player.play(songs, Math.floor(Math.random() * songs.length), true)}
            >
              <ShuffleIcon size={18} />
              {t("music.shuffle")}
            </button>
            {renaming === null && (
              <button type="button" className="button" onClick={() => setRenaming(playlist.name)}>
                <EditIcon size={16} />
                {t("lists.rename")}
              </button>
            )}
            <button type="button" className="button button-quiet" onClick={() => setDeleting(true)}>
              <DeleteIcon size={16} />
              {t("playlists.delete")}
            </button>
          </div>
        </div>
      </header>

      {refused && <p className="notice">{t(refusalKey(refused))}</p>}
      {lines.length === 0 ? (
        <p className="notice">{t("music.playlist_empty")}</p>
      ) : (
        <div className="music-playlist-lines">
          <Sortable
            items={lines}
            keyOf={(line) => line.key}
            nameOf={(line) => line.song.title}
            onMove={(moved) => void change(moved)}
          >
            {(line) => {
              const at = lines.indexOf(line);
              return (
                <div className="music-song music-song-in-a-playlist">
                  <span className="music-song-number">{at + 1}</span>
                  <span className="music-song-words">
                    <button
                      type="button"
                      className="music-song-title music-song-play"
                      onClick={() => player.play(songs, at)}
                      title={t("music.play_song", { title: line.song.title })}
                    >
                      {line.song.title}
                    </button>
                    <span className="music-song-artists">{namesOf(line.song.artists)}</span>
                  </span>
                  <span className="music-song-album">
                    {line.song.album && (
                      <Link to={`/music/album/${line.song.album.id}`}>{line.song.album.name}</Link>
                    )}
                  </span>
                  <Heart id={line.song.id} size={16} />
                  <SongMenuButton
                    songs={[line.song]}
                    label={t("music.more_about", { title: line.song.title })}
                    extra={[
                      {
                        key: "take_out",
                        said: t("music.take_out_of_playlist"),
                        mark: <DeleteIcon size={17} />,
                        act: () => void change(lines.filter((one) => one !== line)),
                      },
                    ]}
                  />
                  <span className="music-song-length">
                    {line.song.seconds === null ? "" : asClock(line.song.seconds)}
                  </span>
                </div>
              );
            }}
          </Sortable>
        </div>
      )}

      {deleting && (
        <Modal
          title={t("lists.delete_title", { name: playlist.name })}
          onClose={() => setDeleting(false)}
          footer={
            <button
              className="button button-accent"
              onClick={() =>
                void attempt(async () => {
                  await music.deletePlaylist(id);
                  playlistsHaveMoved();
                  navigate(-1);
                }).then(() => setDeleting(false))
              }
            >
              {t("playlists.delete")}
            </button>
          }
        >
          <p>{t("music.playlist_delete_why")}</p>
        </Modal>
      )}
    </main>
  );
}
