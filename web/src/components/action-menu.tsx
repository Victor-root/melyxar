/*
 * The list of what can be done with something, as every menu of this
 * interface that hangs from a button draws it: the card's, the page of a
 * work's, a song's.
 *
 * Under a pointer it is one of the bar's lists, drawn over the page at the
 * place of the button and placed once it knows how tall it came out. On a
 * phone it is a sheet from the foot of the screen, where a thumb reaches,
 * with each line tall enough to press.
 */

import { Fragment, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { PHONE, useMediaQuery } from "../media-query";
import { useSettings } from "../settings";
import { BottomSheet } from "./bottom-sheet";

/** One line of a menu. */
export interface MenuEntry {
  key: string;
  /** Its shape, which is what the eye finds before the words are read. */
  mark: ReactNode;
  said: string;
  /** Greyed, and saying when under the pointer, where there is no engine
      behind it yet. */
  later?: boolean;
  act?: () => void;
}

/** How far from the edge of the window a menu is allowed to sit. */
const OFF_THE_EDGE = 8;

export function ActionMenu({
  title,
  groups,
  from,
  openedBy,
  onClose,
}: {
  /** What the menu is about, which a sheet says at its head. */
  title: string;
  /** Families of lines with a rule between them; an empty one is left out. */
  groups: MenuEntry[][];
  /** The button it was opened from, which is where it is drawn. */
  from: DOMRect;
  /** That button itself. A press on it is the one press anywhere outside
      this menu that must not close it, because the button is already going
      to: closing here as well shut it and let the button open it again, so
      it never seemed to close at all. */
  openedBy: HTMLElement | null;
  onClose: () => void;
}) {
  const onAPhone = useMediaQuery(PHONE);
  const lines = groups.filter((group) => group.length > 0);
  if (onAPhone) {
    return (
      <BottomSheet title={title} onClose={onClose}>
        {(shut) => (
          <div className="sheet-menu" role="menu">
            <MenuLines groups={lines} onClose={shut} />
          </div>
        )}
      </BottomSheet>
    );
  }
  return <FloatingMenu groups={lines} from={from} openedBy={openedBy} onClose={onClose} />;
}

function FloatingMenu({
  groups,
  from,
  openedBy,
  onClose,
}: {
  groups: MenuEntry[][];
  from: DOMRect;
  openedBy: HTMLElement | null;
  onClose: () => void;
}) {
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
        // Shut the menu and nothing else: the page under it answers to the
        // same key by going back.
        event.stopPropagation();
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

  return createPortal(
    <div
      className="header-menu-list card-menu"
      ref={holder}
      role="menu"
      style={{
        top: at?.top ?? from.bottom + 6,
        left: at?.left ?? from.left,
        // Drawn once it knows where it goes, so nobody sees it land.
        visibility: at ? "visible" : "hidden",
      }}
    >
      <MenuLines groups={groups} onClose={onClose} />
    </div>,
    document.body,
  );
}

function MenuLines({ groups, onClose }: { groups: MenuEntry[][]; onClose: () => void }) {
  const { t } = useSettings();
  return (
    <>
      {groups.map((group, at) => (
        <Fragment key={group[0].key}>
          {at > 0 && <div className="header-menu-divider" aria-hidden="true" />}
          {group.map((entry) => (
            <button
              key={entry.key}
              type="button"
              role="menuitem"
              className={`header-menu-line${entry.later ? " header-menu-later" : ""}`}
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
              {entry.said}
            </button>
          ))}
        </Fragment>
      ))}
    </>
  );
}
