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
import { api } from "../api";
import type { Card } from "../api";
import { useAccount } from "../account";
import { useMarks } from "../marks";
import { useSettings } from "../settings";

/** One line of the menu. */
interface Entry {
  key: string;
  /** Left out entirely when false: a right nobody has is not a grey line. */
  allowed?: boolean;
  /** Greyed with its reason when there is no engine behind it yet. */
  later?: boolean;
  act?: () => void;
}

/** How far from the edge of the window a menu is allowed to sit. */
const OFF_THE_EDGE = 8;

export function CardMenu({
  card,
  from,
  onClose,
}: {
  card: Card;
  /** The button it was opened from, which is where it is drawn. */
  from: DOMRect;
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
      if (!holder.current?.contains(event.target as Node)) {
        onClose();
      }
    };
    const away = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    /* Drawn at a place rather than next to the button, so a page that moves
       under it leaves it behind: it closes instead of floating. */
    document.addEventListener("mousedown", elsewhere);
    document.addEventListener("keydown", away);
    window.addEventListener("scroll", onClose, true);
    window.addEventListener("resize", onClose);
    return () => {
      document.removeEventListener("mousedown", elsewhere);
      document.removeEventListener("keydown", away);
      window.removeEventListener("scroll", onClose, true);
      window.removeEventListener("resize", onClose);
    };
  }, [onClose]);

  const seen = marks.seenOf(card);
  const favourite = marks.favouriteOf(card);
  const playable = card.source !== null && card.kind !== "series";

  const entries: Entry[] = [
    { key: "play", allowed: playable, act: () => navigate(`/work/${card.id}?play`) },
    { key: "play_from_here", allowed: playable, later: true },
    { key: "collection", later: true },
    { key: "playlist", later: true },
    {
      key: favourite ? "unfavourite" : "favourite",
      act: () => marks.setFavourite(card, !favourite),
    },
    {
      key: seen === "watched" ? "mark_unwatched" : "mark_watched",
      act: () => marks.setWatched(card, seen !== "watched"),
    },
    { key: "select", later: true },
    { key: "download", allowed: account?.may_download === true, later: true },
    { key: "edit_metadata", allowed: account?.is_administrator === true, later: true },
    { key: "edit_images", allowed: account?.is_administrator === true, later: true },
    { key: "edit_subtitles", allowed: account?.is_administrator === true, later: true },
    {
      key: "identify",
      allowed: account?.is_administrator === true,
      // The one action of this menu that is wired all the way through, and it
      // lives on the work's own page, which is where the candidates are shown.
      act: () => navigate(`/work/${card.id}`),
    },
    { key: "forget_identity", allowed: account?.is_administrator === true, later: true },
    { key: "refresh", allowed: account?.is_administrator === true, later: true },
    {
      key: "pin",
      allowed: account?.is_administrator === true,
      act: () => void api.setPinned(card.id, true).catch(() => {}),
    },
    { key: "delete", allowed: account?.may_delete === true, later: true },
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
            disabled={entry.later}
            onClick={(event) => {
              event.preventDefault();
              event.stopPropagation();
              entry.act?.();
              onClose();
            }}
          >
            {t(`card.menu.${entry.key}`)}
            {entry.later && <span className="card-menu-later">{t("nav.later")}</span>}
          </button>
        ))}
    </div>,
    document.body,
  );
}
