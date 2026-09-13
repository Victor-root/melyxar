/*
 * A row of cards that scrolls sideways instead of wrapping.
 *
 * A section of a home page is a suggestion, and a suggestion that spills onto
 * four lines stops being one: what was meant to be a glance becomes a page to
 * scroll past before reaching the next section. So the row keeps one line and
 * what does not fit is reached by moving along it.
 *
 * The arrows appear only when there really is something further along, and are
 * measured from where the cards landed rather than from a count: the number
 * that fits changes with every width, and a button that scrolls nowhere is
 * worse than no button.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useSettings } from "../settings";

/** How much of the visible width one press of an arrow moves. */
const ALMOST_A_SCREENFUL = 0.9;

export function Row({ children }: { children: ReactNode }) {
  const { t } = useSettings();
  const track = useRef<HTMLDivElement>(null);
  const [canGoBack, setCanGoBack] = useState(false);
  const [canGoOn, setCanGoOn] = useState(false);

  const measure = useCallback(() => {
    const element = track.current;
    if (!element) {
      return;
    }
    // A pixel of slack: a scroll position is rarely a whole number.
    setCanGoBack(element.scrollLeft > 1);
    setCanGoOn(element.scrollLeft + element.clientWidth < element.scrollWidth - 1);
  }, []);

  useEffect(() => {
    const element = track.current;
    if (!element) {
      return;
    }
    measure();
    // Both matter: the width changes when the window does, and what is in the
    // row changes when a page finishes loading.
    const watcher = new ResizeObserver(measure);
    watcher.observe(element);
    for (const child of Array.from(element.children)) {
      watcher.observe(child);
    }
    return () => watcher.disconnect();
  }, [measure, children]);

  const move = (direction: 1 | -1) => {
    const element = track.current;
    if (!element) {
      return;
    }
    element.scrollBy({
      left: direction * element.clientWidth * ALMOST_A_SCREENFUL,
      behavior: "smooth",
    });
  };

  /* The arrow keys walk the row, the way they walk a grid. Landing on a card
     that is half off the edge would leave the eye somewhere it did not ask to
     be, so the row follows the focus. */
  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    const element = track.current;
    if (!element) {
      return;
    }
    const cards = Array.from(element.querySelectorAll<HTMLElement>("[data-card]"));
    const current = cards.indexOf(document.activeElement as HTMLElement);
    if (current < 0) {
      return;
    }

    let next: number | null = null;
    switch (event.key) {
      case "ArrowRight":
        next = current + 1;
        break;
      case "ArrowLeft":
        next = current - 1;
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = cards.length - 1;
        break;
      default:
        return;
    }

    if (next < 0 || next >= cards.length) {
      return;
    }
    event.preventDefault();
    cards[next].focus();
    cards[next].scrollIntoView({ inline: "nearest", block: "nearest", behavior: "smooth" });
  };

  return (
    <div className="row">
      {canGoBack && (
        <button
          className="row-arrow row-arrow-back"
          onClick={() => move(-1)}
          aria-label={t("row.back")}
        >
          ‹
        </button>
      )}

      <div className="row-track" ref={track} onScroll={measure} onKeyDown={onKeyDown}>
        {children}
      </div>

      {canGoOn && (
        <button
          className="row-arrow row-arrow-on"
          onClick={() => move(1)}
          aria-label={t("row.on")}
        >
          ›
        </button>
      )}
    </div>
  );
}
