/*
 * The window that puts songs at the end of one of this account's playlists,
 * or in a new one made with them: pressing a playlist is adding to it, as
 * the servers people come from do, and a song may go in one twice.
 */

import { useState } from "react";
import { refusalAbout, useAsked } from "../asking";
import { Modal } from "../components/modal";
import { useToast } from "../components/toasts";
import { PlaylistIcon } from "../icons";
import { howMany } from "../readable";
import { useSettings } from "../settings";
import { music } from "./api";
import type { Song } from "./api";
import { useMusicMarks } from "./marks";

export function AddToPlaylist({ songs, onClose }: { songs: Song[]; onClose: () => void }) {
  const { t } = useSettings();
  const toast = useToast();
  const { playlistsAt, playlistsHaveMoved } = useMusicMarks();
  const every = useAsked((signal) => music.playlists(signal), [playlistsAt]);
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);
  const ids = songs.map((song) => song.id);
  const what = songs.length === 1 ? songs[0].title : howMany(songs.length, "music.songs_count", t);

  const done = (name: string) => {
    playlistsHaveMoved();
    toast({ state: "ok", title: t("music.added_to", { what, name }) });
    onClose();
  };
  const refused = (error: unknown) =>
    toast({ state: "trouble", title: t("music.not_added"), detail: t(refusalAbout(error, "playlist")) });

  const addTo = async (id: string, name: string) => {
    setBusy(true);
    try {
      await music.addToPlaylist(id, ids);
      done(name);
    } catch (error) {
      refused(error);
      setBusy(false);
    }
  };
  const create = async () => {
    const name = typed.trim();
    setBusy(true);
    try {
      await music.createPlaylist(name, ids);
      done(name);
    } catch (error) {
      refused(error);
      setBusy(false);
    }
  };

  return (
    <Modal title={t("music.add_to_playlist_of", { what })} onClose={onClose}>
      {every.answer && every.answer.length === 0 && <p className="settings-why">{t("playlist.none_yet")}</p>}
      {every.answer && every.answer.length > 0 && (
        <ul className="lists-lines">
          {every.answer.map((playlist) => (
            <li key={playlist.id}>
              <button
                type="button"
                className="lists-line"
                disabled={busy}
                onClick={() => void addTo(playlist.id, playlist.name)}
              >
                <span className="lists-mark" aria-hidden="true">
                  <PlaylistIcon size={14} />
                </span>
                <span className="lists-name">{playlist.name}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
      <form
        className="lists-new"
        onSubmit={(event) => {
          event.preventDefault();
          void create();
        }}
      >
        <input
          type="text"
          className="field-line"
          aria-label={t("playlist.new")}
          placeholder={t("playlist.new")}
          maxLength={80}
          value={typed}
          onChange={(event) => setTyped(event.target.value)}
        />
        <button type="submit" className="button button-small button-accent" disabled={busy || typed.trim() === ""}>
          {t("lists.create")}
        </button>
      </form>
    </Modal>
  );
}
