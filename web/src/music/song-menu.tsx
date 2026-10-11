/*
 * What a song, or several at once, offers beyond pressing it: to play next
 * or last, to go in a playlist, to reach its album and its artist, and
 * whatever the screen it is on adds, such as taking it out of a playlist.
 *
 * Drawn over the page where its button is, like the menu of a film, and
 * made only when opened: a list of a thousand songs holds a thousand
 * buttons, not a thousand menus.
 */

import { useCallback, useRef, useState } from "react";
import type { ReactNode } from "react";
import { ActionMenu } from "../components/action-menu";
import type { MenuEntry } from "../components/action-menu";
import { MusicIcon, PlayAllIcon, PlaylistIcon, ProfileIcon } from "../icons";
import { useGoTo } from "../navigating";
import { useSettings } from "../settings";
import type { Song } from "./api";
import { AddToPlaylist } from "./add-to-playlist";
import { useSongDeletion } from "./delete-song";
import { useSongLyrics } from "./lyrics-dialog";
import { QueueIcon } from "./player/icons";
import { useMusicControls } from "./player/player";

/** A line a screen adds to the menu. */
export type MenuLine = MenuEntry & { act: () => void };

/** What can be done with songs beyond pressing them, in the order a menu and
 *  a line of a list both offer them, and the windows that adding to a
 *  playlist, choosing lyrics and deleting open, to be drawn wherever the
 *  actions are. */
export function useSongActions(songs: Song[], deletable = false): { actions: MenuLine[]; dialog: ReactNode } {
  const { t } = useSettings();
  const navigate = useGoTo();
  const player = useMusicControls();
  const [adding, setAdding] = useState(false);
  const one = songs.length === 1 ? songs[0] : null;
  const deletion = useSongDeletion(deletable ? songs : []);
  const lyrics = useSongLyrics(songs);
  const actions: MenuLine[] = [
    { key: "next", said: t("music.play_next"), mark: <PlayAllIcon size={17} />, act: () => player.playNext(songs) },
    { key: "last", said: t("music.play_last"), mark: <QueueIcon size={17} />, act: () => player.playLast(songs) },
    { key: "playlist", said: t("music.add_to_playlist"), mark: <PlaylistIcon size={17} />, act: () => setAdding(true) },
    ...(one?.album
      ? [{ key: "album", said: t("music.go_to_album"), mark: <MusicIcon size={17} />, act: () => navigate(`/music/album/${one.album!.id}`) }]
      : []),
    ...(one && one.artists.length > 0
      ? [{ key: "artist", said: t("music.go_to_artist"), mark: <ProfileIcon size={17} />, act: () => navigate(`/music/artist/${one.artists[0].id}`) }]
      : []),
    ...lyrics.lines,
    ...deletion.lines,
  ];
  return {
    actions,
    dialog: (
      <>
        {adding && <AddToPlaylist songs={songs} onClose={() => setAdding(false)} />}
        {lyrics.dialog}
        {deletion.dialog}
      </>
    ),
  };
}

export function SongMenuButton({
  songs,
  label,
  extra = [],
  className = "",
  inline = 0,
  deletable = false,
}: {
  songs: Song[];
  label: string;
  extra?: MenuLine[];
  className?: string;
  /** How many of the first actions a line already carries on itself, and
      that the menu therefore leaves out. */
  inline?: number;
  /** Whether the menu of this one song offers to delete it: its line in a
      list does, the menu of a whole album or playlist does not. */
  deletable?: boolean;
}) {
  const [from, setFrom] = useState<DOMRect | null>(null);
  const button = useRef<HTMLButtonElement>(null);
  const close = useCallback(() => setFrom(null), []);
  const { actions, dialog } = useSongActions(songs, deletable);
  const lines = [...actions.slice(inline), ...extra];
  // A line that carries every action itself has nothing left for a menu.
  if (lines.length === 0) {
    return <>{dialog}</>;
  }
  return (
    <>
      <button
        ref={button}
        type="button"
        className={`music-more${from ? " music-more-open" : ""}${className ? ` ${className}` : ""}`}
        aria-label={label}
        title={label}
        aria-expanded={from !== null}
        onClick={(event) => {
          event.stopPropagation();
          setFrom(from ? null : event.currentTarget.getBoundingClientRect());
        }}
      >
        <span aria-hidden="true">⋯</span>
      </button>
      {from && (
        <ActionMenu
          title={songs.length === 1 ? songs[0].title : label}
          groups={[lines]}
          from={from}
          openedBy={button.current}
          onClose={close}
        />
      )}
      {dialog}
    </>
  );
}
