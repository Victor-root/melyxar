/*
 * A sheet that comes up from the foot of the screen on a phone, for a choice
 * that a floating list has no room for under a thumb.
 *
 * Shut by a press on the dimmed page above it, by the escape key, and by
 * pulling its head down: it follows the finger and goes if it was pulled far
 * enough or fast enough, and goes back up otherwise. Its body scrolls on its
 * own, so the head stays where it was put however long what is in it turns
 * out to be, under the thin line the pages of a phone show while they move.
 *
 * Drawn over the page rather than inside it, as the other panels are, so
 * nothing above it clips it. It moves by transform alone: the pull is written
 * on the element as the finger moves, not through the component, so nothing is
 * drawn again for it.
 */

import { useEffect, useRef } from "react";
import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { useHoldsTheScreen } from "./hold-the-screen";
import { ScrollBar } from "./scrollbar";

/** How long the sheet takes to leave, in either direction of its being shut. */
const LEAVES_IN_MS = 220;
/** How far down it has to be pulled to go, at most, and as a share of its own
 *  height when that is less. */
const PULL_TO_CLOSE = 140;
const PULL_SHARE = 0.3;
/** How fast a short pull has to be, in pixels a millisecond, to go anyway. */
const FLICK_SPEED = 0.6;

export function BottomSheet({
  title,
  onClose,
  className,
  children,
}: {
  title: string;
  onClose: () => void;
  /** What kind of sheet it is, for one shaped differently from the rest. */
  className?: string;
  /** What is in it, or a function of the way to shut it, for a line that
   *  shuts the sheet once it has done what it was pressed for. */
  children: ReactNode | ((shut: () => void) => ReactNode);
}) {
  const over = useRef<HTMLDivElement>(null);
  const sheet = useRef<HTMLDivElement>(null);
  const body = useRef<HTMLDivElement>(null);
  const pull = useRef<{ from: number; at: number } | null>(null);
  const leaving = useRef(0);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;

  /* Slid down, then told. Reached by every way of shutting it, so that all of
     them look the same. */
  const shut = () => {
    const element = sheet.current;
    if (!element || leaving.current) {
      return;
    }
    const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const ms = still ? 0 : LEAVES_IN_MS;
    element.style.animation = "none";
    element.style.transition = `transform ${ms}ms var(--ease)`;
    element.style.transform = "translateY(100%)";
    over.current?.setAttribute("data-leaving", "");
    leaving.current = window.setTimeout(() => closeRef.current(), ms);
  };

  useEffect(() => () => window.clearTimeout(leaving.current), []);
  useHoldsTheScreen(sheet, shut);

  const settle = (element: HTMLElement, to: string) => {
    element.style.transition = `transform ${LEAVES_IN_MS}ms var(--ease)`;
    element.style.transform = to;
  };

  return createPortal(
    <div
      ref={over}
      className="sheet-over"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) {
          shut();
        }
      }}
    >
      <div
        ref={sheet}
        className={className ? `sheet ${className}` : "sheet"}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        tabIndex={-1}
      >
        <div
          className="sheet-grip"
          onPointerDown={(event) => {
            event.currentTarget.setPointerCapture(event.pointerId);
            const element = sheet.current;
            if (element) {
              element.style.animation = "none";
              element.style.transition = "none";
            }
            pull.current = { from: event.clientY, at: event.timeStamp };
          }}
          onPointerMove={(event) => {
            const started = pull.current;
            if (started && sheet.current) {
              sheet.current.style.transform = `translateY(${Math.max(0, event.clientY - started.from)}px)`;
            }
          }}
          onPointerUp={(event) => {
            const started = pull.current;
            const element = sheet.current;
            pull.current = null;
            if (!started || !element) {
              return;
            }
            const down = Math.max(0, event.clientY - started.from);
            const speed = down / Math.max(1, event.timeStamp - started.at);
            if (down > Math.min(PULL_TO_CLOSE, element.offsetHeight * PULL_SHARE) || speed > FLICK_SPEED) {
              shut();
            } else {
              settle(element, "translateY(0)");
            }
          }}
          onPointerCancel={() => {
            pull.current = null;
            if (sheet.current) {
              settle(sheet.current, "translateY(0)");
            }
          }}
        >
          <span className="sheet-handle" aria-hidden="true" />
          <h2 className="sheet-title">{title}</h2>
        </div>
        <div className="sheet-scroll">
          <div className="sheet-body" ref={body}>
            {typeof children === "function" ? children(shut) : children}
          </div>
          <ScrollBar holder={body} />
        </div>
      </div>
    </div>,
    document.body,
  );
}
