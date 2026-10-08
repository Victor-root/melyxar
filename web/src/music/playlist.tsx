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
import { useNavigate, useParams } from "react-router-dom";
import { refusalOf } from "../asking";
import { Modal } from "../components/modal";
import { Sortable } from "../components/sortable";
import { refusalKey } from "../i18n";
import { DeleteIcon, EditIcon, PlayIcon } from "../icons";
import { PHONE, useMediaQuery } from "../media-query";
import { howMany } from "../readable";
import { useSettings } from "../settings";
import { music } from "./api";
import type { MusicPlaylistPage, Song } from "./api";
import { minutesOf } from "./discs";
import { useMusicMarks } from "./marks";
import { ShuffleIcon } from "./player/icons";
import { useMusicControls } from "./player/player";
import { SongMenuButton } from "./song-menu";
import type { MenuLine } from "./song-menu";
import { SongLine } from "./songs";
import { PlaylistCover } from "./tiles";
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
  const player = useMusicControls();
  const { playlistsAt, playlistsHaveMoved } = useMusicMarks();
  const [playlist, setPlaylist] = useState<MusicPlaylistPage | null>(null);
  useTabPage(playlist?.name);
  const [lines, setLines] = useState<Line[]>([]);
  const [failed, setFailed] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);
  const named = useRef(0);
  /* On a phone the lines are those of every other list of songs there, and
     renaming and deleting go into the playlist's menu, as an album's tools
     do into its own. */
  const phone = useMediaQuery(PHONE);

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
  const play = (at: number) => player.play(songs, at);
  const inTheMenu: MenuLine[] = phone
    ? [
        ...(renaming === null
          ? [{ key: "rename", said: t("lists.rename"), mark: <EditIcon size={17} />, act: () => setRenaming(playlist.name) }]
          : []),
        { key: "delete", said: t("playlists.delete"), mark: <DeleteIcon size={17} />, act: () => setDeleting(true) },
      ]
    : [];
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
            {phone ? (
              <SongMenuButton
                songs={songs}
                label={t("music.more_about", { title: playlist.name })}
                className="music-hero-heart"
                extra={inTheMenu}
              />
            ) : (
              <>
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
              </>
            )}
          </div>
        </div>
      </header>

      {refused && <p className="notice">{t(refusalKey(refused))}</p>}
      {lines.length === 0 ? (
        <p className="notice">{t("music.playlist_empty")}</p>
      ) : (
        <div className={`music-playlist-lines music-songs${phone ? " music-songs-compact" : ""}`}>
          <Sortable
            items={lines}
            keyOf={(line) => line.key}
            nameOf={(line) => line.song.title}
            onMove={(moved) => void change(moved)}
          >
            {(line) => (
              <SongLine
                song={line.song}
                index={lines.indexOf(line)}
                first={0}
                numbered="place"
                showAlbum
                compact={phone}
                inline={0}
                onPlay={play}
                inItsOwnLine
                extra={[
                  {
                    key: "take_out",
                    said: t("music.take_out_of_playlist"),
                    mark: <DeleteIcon size={17} />,
                    act: () => void change(lines.filter((one) => one !== line)),
                  },
                ]}
              />
            )}
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
