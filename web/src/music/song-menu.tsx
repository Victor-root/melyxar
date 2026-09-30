/*
 * What a song, or several at once, offers beyond pressing it: to play next
 * or last, to go in a playlist, to reach its album and its artist, and
 * whatever the screen it is on adds, such as taking it out of a playlist.
 *
 * Drawn over the page where its button is, like the menu of a film, and
 * made only when opened: a list of a thousand songs holds a thousand
 * buttons, not a thousand menus.
 */

import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { useNavigate } from "react-router-dom";
import { MusicIcon, PlayAllIcon, PlaylistIcon, ProfileIcon } from "../icons";
import { useSettings } from "../settings";
import type { Song } from "./api";
import { AddToPlaylist } from "./add-to-playlist";
import { QueueIcon } from "./player/icons";
import { useMusic } from "./player/player";

/** A line a screen adds to the menu. */
export interface MenuLine {
  key: string;
  said: string;
  mark: ReactNode;
  act: () => void;
}

/** How far from the edge of the window a menu is allowed to sit. */
const OFF_THE_EDGE = 8;

export function SongMenuButton({
  songs,
  label,
  extra = [],
  className = "",
}: {
  songs: Song[];
  label: string;
  extra?: MenuLine[];
  className?: string;
}) {
  const [from, setFrom] = useState<DOMRect | null>(null);
  const [adding, setAdding] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  const close = useCallback(() => setFrom(null), []);
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
        <SongMenu
          songs={songs}
          from={from}
          openedBy={button.current}
          extra={extra}
          onAdd={() => setAdding(true)}
          onClose={close}
        />
      )}
      {adding && <AddToPlaylist songs={songs} onClose={() => setAdding(false)} />}
    </>
  );
}

function SongMenu({
  songs,
  from,
  openedBy,
  extra,
  onAdd,
  onClose,
}: {
  songs: Song[];
  from: DOMRect;
  openedBy: HTMLElement | null;
  extra: MenuLine[];
  onAdd: () => void;
  onClose: () => void;
}) {
  const { t } = useSettings();
  const navigate = useNavigate();
  const player = useMusic();
  const holder = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState<{ top: number; left: number } | null>(null);

  useLayoutEffect(() => {
    const menu = holder.current;
    if (!menu) {
      return;
    }
    const { width, height } = menu.getBoundingClientRect();
    const under = from.bottom + 6;
    const top =
      under + height > window.innerHeight - OFF_THE_EDGE ? Math.max(OFF_THE_EDGE, from.top - height - 6) : under;
    const left = Math.min(Math.max(OFF_THE_EDGE, from.right - width), window.innerWidth - width - OFF_THE_EDGE);
    setAt({ top, left });
  }, [from]);

  useEffect(() => {
    const elsewhere = (event: MouseEvent) => {
      const where = event.target as Node;
      if (!holder.current?.contains(where) && !openedBy?.contains(where)) {
        onClose();
      }
    };
    const away = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        onClose();
      }
    };
    const moved = (event: Event) => {
      if (!holder.current?.contains(event.target as Node)) {
        onClose();
      }
    };
    document.addEventListener("mousedown", elsewhere);
    document.addEventListener("keydown", away);
    window.addEventListener("scroll", moved, true);
    window.addEventListener("resize", onClose);
    return () => {
      document.removeEventListener("mousedown", elsewhere);
      document.removeEventListener("keydown", away);
      window.removeEventListener("scroll", moved, true);
      window.removeEventListener("resize", onClose);
    };
  }, [onClose, openedBy]);

  const one = songs.length === 1 ? songs[0] : null;
  const lines: MenuLine[] = [
    { key: "next", said: t("music.play_next"), mark: <PlayAllIcon size={17} />, act: () => player.playNext(songs) },
    { key: "last", said: t("music.play_last"), mark: <QueueIcon size={17} />, act: () => player.playLast(songs) },
    { key: "playlist", said: t("music.add_to_playlist"), mark: <PlaylistIcon size={17} />, act: onAdd },
    ...(one?.album
      ? [{ key: "album", said: t("music.go_to_album"), mark: <MusicIcon size={17} />, act: () => navigate(`/music/album/${one.album!.id}`) }]
      : []),
    ...(one && one.artists.length > 0
      ? [{ key: "artist", said: t("music.go_to_artist"), mark: <ProfileIcon size={17} />, act: () => navigate(`/music/artist/${one.artists[0].id}`) }]
      : []),
    ...extra,
  ];

  return createPortal(
    <div
      className="header-menu-list card-menu"
      ref={holder}
      role="menu"
      style={{ top: at?.top ?? from.bottom + 6, left: at?.left ?? from.left, visibility: at ? "visible" : "hidden" }}
    >
      {lines.map((line) => (
        <button
          key={line.key}
          type="button"
          role="menuitem"
          className="header-menu-line"
          onClick={(event) => {
            event.preventDefault();
            event.stopPropagation();
            line.act();
            onClose();
          }}
        >
          {line.mark}
          {line.said}
        </button>
      ))}
    </div>,
    document.body,
  );
}
