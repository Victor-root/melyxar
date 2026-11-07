/*
 * A list that a phone lets down from the top of the screen, as its own
 * notifications are: a title with the way out above, and a handle at the foot
 * that is pulled up. Both are drawn only there; the list itself is a Dropdown
 * wearing the `top-sheet` class, which is shut by any press in it.
 */

import { useRef } from "react";
import type { ReactNode } from "react";
import { CloseIcon } from "../icons";
import { useSettings } from "../settings";

/** How far up the handle has to be pulled to put the sheet away, at most: a
 *  short sheet goes with a third of its height. */
const PULL_TO_CLOSE = 120;

/** How far a finger may wander on the handle and still be pressing it. */
const PRESS_SLACK = 6;

/** How long the sheet takes to go, or to come back, once let go of. */
const LETS_GO_MS = 180;

/** The title of the sheet, after what `children` draws at its left. */
export function TopSheetHead({ title, children }: { title: string; children: ReactNode }) {
  const { t } = useSettings();
  return (
    <div className="top-sheet-head">
      {children}
      <span className="top-sheet-title">{title}</span>
      <button type="button" className="top-sheet-close" aria-label={t("modal.close")}>
        <CloseIcon size={18} />
      </button>
    </div>
  );
}

/** The handle at the foot of the sheet: held and pulled up, the sheet follows
 *  the finger and goes if it was pulled far enough; pressed, it goes at once. */
export function TopSheetHandle() {
  const { t } = useSettings();
  /* The pull in progress: where the finger went down and the sheet it is
     moving. */
  const pulled = useRef<{ from: number; sheet: HTMLElement; moved: boolean } | null>(null);

  return (
    <button
      type="button"
      className="top-sheet-handle"
      aria-label={t("modal.close")}
      onPointerDown={(event) => {
        const sheet = event.currentTarget.closest<HTMLElement>(".top-sheet");
        if (!sheet) {
          return;
        }
        event.currentTarget.setPointerCapture(event.pointerId);
        sheet.style.animation = "none";
        sheet.style.transition = "none";
        pulled.current = { from: event.clientY, sheet, moved: false };
      }}
      onPointerMove={(event) => {
        const pull = pulled.current;
        if (!pull) {
          return;
        }
        const up = Math.min(0, event.clientY - pull.from);
        pull.moved = pull.moved || up < -PRESS_SLACK;
        pull.sheet.style.transform = `translateY(${up}px)`;
      }}
      onPointerUp={(event) => {
        const pull = pulled.current;
        if (!pull) {
          return;
        }
        const handle = event.currentTarget;
        const up = event.clientY - pull.from;
        pull.sheet.style.transition = `transform ${LETS_GO_MS}ms ease-out`;
        if (up < -Math.min(PULL_TO_CLOSE, pull.sheet.offsetHeight * 0.3)) {
          pull.sheet.style.transform = "translateY(-100%)";
          /* The click the browser sends after this is the end of the pull
             and is swallowed; the one that shuts the sheet is sent once it
             has gone. */
          pull.moved = true;
          window.setTimeout(() => {
            pulled.current = null;
            handle.click();
          }, LETS_GO_MS);
          return;
        }
        pull.sheet.style.transform = "";
      }}
      onPointerCancel={() => {
        const pull = pulled.current;
        pulled.current = null;
        if (pull) {
          pull.sheet.style.transition = `transform ${LETS_GO_MS}ms ease-out`;
          pull.sheet.style.transform = "";
        }
      }}
      onClick={(event) => {
        /* The press that ends a pull is not a press on the handle. */
        const pull = pulled.current;
        pulled.current = null;
        if (pull?.moved) {
          event.stopPropagation();
        }
      }}
    >
      <span aria-hidden="true" />
    </button>
  );
}
