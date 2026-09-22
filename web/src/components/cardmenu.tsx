/*
 * What a card offers beyond pressing it.
 *
 * Every action this interface is meant to have is listed, including the ones
 * the engine cannot do yet. Those are drawn greyed and say when rather than
 * being left out: a function nobody can see is a function nobody knows is
 * coming, and the shape of the menu stops moving under people's hands as each
 * one is wired.
 *
 * What is hidden rather than greyed is what this account has no right to. A
 * greyed entry says "later"; an entry somebody will never be allowed to press
 * says nothing worth saying, and saying it tells them what the server holds.
 *
 * Drawn over the page rather than inside the card. A card clips what it
 * holds, both to round its picture and because the browser is told it may
 * skip drawing the ones off screen, and a menu drawn inside one comes out cut
 * in half. So it is placed where the button is and drawn at the top of the
 * page, which is also what keeps it above the cards after it.
 */

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useNavigate } from "react-router-dom";
import type { Card } from "../api";
import { useAccount } from "../account";
import { useMarks } from "../marks";
import { useSettings } from "../settings";
import { isCatalogued, playsOnItsOwn } from "../works";
import {
  CollectionIcon,
  DeleteIcon,
  DownloadIcon,
  EditIcon,
  ForgetIcon,
  HeartIcon,
  IdentifyIcon,
  ImageIcon,
  PinIcon,
  PlayAllIcon,
  PlayIcon,
  PlaylistIcon,
  RefreshIcon,
  SelectIcon,
  SubtitlesIcon,
  TickIcon,
} from "../icons";

/** How big the shape at the left of a line is. */
const SHAPE = 17;

/** One line of the menu. */
interface Entry {
  key: string;
  /** Its shape, which is what the eye finds before the words are read. */
  mark: React.ReactNode;
  /** Left out entirely when false: a right nobody has is not a grey line. */
  allowed?: boolean;
  /** Greyed, and saying when under the pointer, where there is no engine
      behind it yet. Written beside every such line it became a second column
      of the same three words repeated down the menu. */
  later?: boolean;
  act?: () => void;
}

/** How far from the edge of the window a menu is allowed to sit. */
const OFF_THE_EDGE = 8;

export function CardMenu({
  card,
  from,
  openedBy,
  onIdentify,
  onEditImages,
  onClose,
}: {
  card: Card;
  /** The button it was opened from, which is where it is drawn. */
  from: DOMRect;
  /** That button itself. A press on it is the one press anywhere outside
      this menu that must not close it, because the button is already going
      to: closing here as well shut it and let the button open it again, so
      it never seemed to close at all. */
  openedBy: HTMLElement | null;
  /** Opens the panel that says by hand what this work is. Asked of whoever
      drew this menu rather than opened from here: this menu is taken away the
      moment anything in it is pressed, and a panel drawn inside it would go
      with it. */
  onIdentify: () => void;
  /** Opens the panel that chooses the pictures this work wears, for the same
      reason as the one above. */
  onEditImages: () => void;
  onClose: () => void;
}) {
  const { t } = useSettings();
  const navigate = useNavigate();
  const { account } = useAccount();
  const marks = useMarks();
  const holder = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState<{ top: number; left: number } | null>(null);

  // Placed once it has been drawn, since where it fits depends on how tall it
  // came out: above the button when there is no room under it, and pulled
  // back from the edge rather than hanging off it.
  useLayoutEffect(() => {
    const menu = holder.current;
    if (!menu) {
      return;
    }
    const { width, height } = menu.getBoundingClientRect();
    const under = from.bottom + 6;
    const top =
      under + height > window.innerHeight - OFF_THE_EDGE
        ? Math.max(OFF_THE_EDGE, from.top - height - 6)
        : under;
    const left = Math.min(
      Math.max(OFF_THE_EDGE, from.right - width),
      window.innerWidth - width - OFF_THE_EDGE,
    );
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
        onClose();
      }
    };
    /* Drawn at a place rather than next to the button, so a page that moves
       under it leaves it behind: it closes instead of floating. Its own
       scrolling is not the page moving, though, and watching every scroll in
       the document made a menu too tall for the window close the moment
       anybody reached for what was below the fold. */
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

  const seen = marks.seenOf(card);
  const favourite = marks.favouriteOf(card);
  const pinned = marks.pinnedOf(card) === true;
  const playable = playsOnItsOwn(card);
  const catalogued = isCatalogued(card.identification);

  const entries: Entry[] = [
    {
      key: "play",
      mark: <PlayIcon size={SHAPE} />,
      allowed: playable,
      act: () => navigate(`/work/${card.id}?play`),
    },
    {
      key: "play_from_here",
      mark: <PlayAllIcon size={SHAPE} />,
      allowed: playable,
      later: true,
    },
    { key: "collection", mark: <CollectionIcon size={SHAPE} />, later: true },
    { key: "playlist", mark: <PlaylistIcon size={SHAPE} />, later: true },
    {
      key: favourite ? "unfavourite" : "favourite",
      mark: <HeartIcon size={SHAPE} filled={favourite} />,
      act: () => marks.setFavourite(card, !favourite),
    },
    {
      key: seen === "watched" ? "mark_unwatched" : "mark_watched",
      mark: <TickIcon size={SHAPE} />,
      act: () => marks.setWatched(card, seen !== "watched"),
    },
    { key: "select", mark: <SelectIcon size={SHAPE} />, later: true },
    {
      key: "download",
      mark: <DownloadIcon size={SHAPE} />,
      allowed: account?.may_download === true,
      later: true,
    },
    {
      key: "edit_metadata",
      mark: <EditIcon size={SHAPE} />,
      allowed: account?.is_administrator === true,
      later: true,
    },
    {
      key: "edit_images",
      mark: <ImageIcon size={SHAPE} />,
      allowed: account?.is_administrator === true && catalogued,
      act: onEditImages,
    },
    {
      key: "edit_subtitles",
      mark: <SubtitlesIcon size={SHAPE} />,
      allowed: account?.is_administrator === true,
      later: true,
    },
    {
      key: "identify",
      mark: <IdentifyIcon size={SHAPE} />,
      allowed: account?.is_administrator === true && catalogued,
      act: onIdentify,
    },
    {
      key: "forget_identity",
      mark: <ForgetIcon size={SHAPE} />,
      allowed: account?.is_administrator === true,
      later: true,
    },
    {
      key: "refresh",
      mark: <RefreshIcon size={SHAPE} />,
      allowed: account?.is_administrator === true,
      later: true,
    },
    {
      /* Says what it will do rather than always the same thing. Nothing is
         known about a card that arrives from the server, so it offers to put
         it there; once it has been put there from here, it offers to take it
         back off. */
      key: pinned ? "unpin" : "pin",
      mark: <PinIcon size={SHAPE} filled={pinned} />,
      allowed: account?.is_administrator === true,
      act: () => marks.setPinned(card, !pinned),
    },
    {
      key: "delete",
      mark: <DeleteIcon size={SHAPE} />,
      allowed: account?.may_delete === true,
      later: true,
    },
  ];

  return createPortal(
    <div
      className="card-menu"
      ref={holder}
      role="menu"
      style={{
        top: at?.top ?? from.bottom + 6,
        left: at?.left ?? from.left,
        // Drawn once it knows where it goes, so nobody sees it land.
        visibility: at ? "visible" : "hidden",
      }}
    >
      {entries
        .filter((entry) => entry.allowed !== false)
        .map((entry) => (
          <button
            key={entry.key}
            type="button"
            role="menuitem"
            className="card-menu-line"
            /* Said rather than disabled outright: a disabled button takes no
               pointer, so the very tooltip that explains why it is grey
               never appears. */
            aria-disabled={entry.later}
            title={entry.later ? t("nav.later") : undefined}
            onClick={(event) => {
              event.preventDefault();
              event.stopPropagation();
              if (entry.later) {
                return;
              }
              entry.act?.();
              onClose();
            }}
          >
            {entry.mark}
            {t(`card.menu.${entry.key}`)}
          </button>
        ))}
    </div>,
    document.body,
  );
}
