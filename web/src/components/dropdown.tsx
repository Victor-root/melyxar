import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ChevronDownIcon } from "../icons";

/**
 * A word that opens a short list under it.
 *
 * Closes on a click anywhere else and on the escape key, which are the two
 * ways anybody ever tries to close one.
 *
 * The list is drawn at the end of the page rather than inside the bar, and
 * placed against the window by hand. That is not a preference: the pieces
 * of the bar are frosted glass, and an element that frosts what is behind
 * it becomes the backdrop of everything inside it. A list that frosts the
 * page from in there frosts the inside of its own piece, which is nothing
 * at all, and comes out as clear glass over whatever film is playing
 * underneath. Outside it, there is a page behind it to frost.
 */
export function Dropdown({
  label,
  className,
  reachable,
  icon,
  listClassName,
  placed = true,
  onOpen,
  children,
}: {
  label: React.ReactNode;
  /** What this one is, for the few that are not a word in the bar. */
  className?: string;
  /** Whether what holds it is open. Folded away, the tab key passes it by
   *  and any list it had left hanging is shut. */
  reachable: boolean;
  /** Opened by an icon of the bar rather than a word: drawn as the other
   *  icons are, without the fold, and named for whoever cannot see it. */
  icon?: string;
  /** What the list is, for one that holds more than short lines. */
  listClassName?: string;
  /** Whether the list is put under what opened it. Said no where the style
   *  sheet places it itself, as a sheet over the whole width on a phone. */
  placed?: boolean;
  /** Told each time the list is opened. */
  onOpen?: () => void;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const holder = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);
  /** Where the list goes: under what opened it, and ending where it ends. */
  const [under, setUnder] = useState({ top: 0, right: 0 });

  /* Measured when it opens and again if the window changes shape. The bar it
     hangs from is fixed to the window, so a page scrolled underneath moves
     nothing here. */
  useLayoutEffect(() => {
    if (!open) {
      return;
    }
    const place = () => {
      const it = holder.current?.getBoundingClientRect();
      if (it) {
        setUnder({ top: it.bottom + 6, right: Math.max(window.innerWidth - it.right, 0) });
      }
    };
    place();
    window.addEventListener("resize", place);
    return () => window.removeEventListener("resize", place);
  }, [open]);

  useEffect(() => {
    if (!open) {
      return;
    }
    /* The list is no longer inside what opened it, so a press in it is a
       press outside as far as the page is concerned: both are asked. */
    const elsewhere = (event: MouseEvent) => {
      const on = event.target as Node;
      if (!holder.current?.contains(on) && !list.current?.contains(on)) {
        setOpen(false);
      }
    };
    const away = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", elsewhere);
    document.addEventListener("keydown", away);
    return () => {
      document.removeEventListener("mousedown", elsewhere);
      document.removeEventListener("keydown", away);
    };
  }, [open]);

  /* Shut along with whatever folded it away, or it would be left hanging
     over a field that is no longer there. */
  useEffect(() => {
    if (!reachable) {
      setOpen(false);
    }
  }, [reachable]);

  return (
    <div className={`header-menu${className ? ` ${className}` : ""}`} ref={holder}>
      <button
        type="button"
        className={`${icon ? "header-icon" : "header-link"}${open ? " header-link-on" : ""}`}
        aria-expanded={open}
        aria-label={icon}
        title={icon}
        tabIndex={reachable ? undefined : -1}
        onClick={() => {
          if (!open) onOpen?.();
          setOpen(!open);
        }}
      >
        {label}
        {!icon && <ChevronDownIcon size={15} />}
      </button>
      {open &&
        createPortal(
          <div
            className={`header-menu-list${listClassName ? ` ${listClassName}` : ""}`}
            ref={list}
            style={placed ? { top: under.top, right: under.right } : undefined}
            /* A press in the list leaves the focus where it was. Drawn at
               the end of the page, the list is outside what opened it, and
               a press taking the focus there read as leaving: the search
               field, empty, folded away with its scope list under the hand
               choosing from it. */
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => setOpen(false)}
          >
            {children}
          </div>,
          document.body,
        )}
    </div>
  );
}
