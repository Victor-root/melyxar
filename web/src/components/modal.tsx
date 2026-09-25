/*
 * A panel that takes the screen while something is being decided.
 *
 * What goes in one: a job with a beginning and an end that the page under it
 * has no room for, and that whoever started it has to be able to leave without
 * having changed anything. Correcting what a film is, or which pictures it
 * wears, are both exactly that.
 *
 * Drawn over the page rather than inside it, so nothing above it clips it and
 * the row it was opened from keeps its place. Shut by the cross, by the key
 * every panel in the world is shut with, and by a press outside it.
 *
 * The page behind stops scrolling while it is open: a wheel turned over a
 * panel that scrolls the library behind it is the one thing that says "this is
 * not really a window".
 */

import { useEffect, useRef } from "react";
import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { BackIcon, CloseIcon } from "../icons";
import { useSettings } from "../settings";

export function Modal({
  title,
  /** Drawn at the left of the title, where a step back belongs: the panels
      here have steps, and a cross on a second step would throw away the work
      of the first. */
  onBack,
  onClose,
  /** The row along the foot, which holds the one action the panel is for.
      Nothing when the panel is only read, or when what it holds is chosen by
      pressing it rather than by agreeing to it. */
  footer,
  /** What kind of panel it is, for one shaped differently from the rest. */
  className,
  /** Where it is drawn, when not over the whole page: a panel opened while
      something else fills the screen has to be drawn inside that. */
  into,
  children,
}: {
  title: string;
  onBack?: () => void;
  onClose: () => void;
  footer?: ReactNode;
  className?: string;
  into?: Element;
  children: ReactNode;
}) {
  const { t } = useSettings();
  const panel = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const shut = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        onClose();
      }
    };
    document.addEventListener("keydown", shut);
    /* The page underneath holds still. Put back as it was rather than set to
       a value: another panel may have set it first, and two panels closing
       one after the other would otherwise leave the page unable to scroll. */
    const before = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    panel.current?.focus();
    return () => {
      document.removeEventListener("keydown", shut);
      document.body.style.overflow = before;
    };
  }, [onClose]);

  return createPortal(
    <div
      className="modal-over"
      /* A press that starts and ends outside the panel shuts it. Started
         inside and finished outside, which is what a drag across a field
         does, it does not: losing a form to a slip of the hand is worse than
         a panel that stays a second too long. */
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) {
          onClose();
        }
      }}
    >
      <div
        className={className ? `modal ${className}` : "modal"}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        ref={panel}
        tabIndex={-1}
      >
        <header className="modal-head">
          {onBack && (
            <button className="modal-step" onClick={onBack} aria-label={t("modal.back")}>
              <BackIcon size={22} />
            </button>
          )}
          <h2 className="modal-title">{title}</h2>
          <button className="modal-step" onClick={onClose} aria-label={t("modal.close")}>
            <CloseIcon size={22} />
          </button>
        </header>

        <div className="modal-body">{children}</div>

        {footer && <footer className="modal-foot">{footer}</footer>}
      </div>
    </div>,
    into ?? document.body,
  );
}
