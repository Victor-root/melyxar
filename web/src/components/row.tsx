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
import { Link } from "react-router-dom";
import { useDragToScroll } from "../dragging";
import { useSettings } from "../settings";
import { ChevronLeftIcon, ChevronRightIcon } from "../icons";

/** How much of the visible width one press of an arrow moves, when the row
 *  holds too few cards to be stepped card by card. */
const ALMOST_A_SCREENFUL = 0.9;

/** A hair of slack: a scroll position and a laid out edge are rarely whole
 *  numbers, and a row sitting a tenth of a pixel short of its place must
 *  still count as being in it. */
const A_HAIR = 0.5;

export function Row({ children }: { children: ReactNode }) {
  const { t } = useSettings();
  const track = useRef<HTMLDivElement>(null);
  const [canGoBack, setCanGoBack] = useState(false);
  const [canGoOn, setCanGoOn] = useState(false);
  // Held down and pulled, the way the player's own rows already worked.
  const drag = useDragToScroll(track);

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

  /* One press moves the row by whole cards and leaves it on a card's edge.
     Scrolling by a share of the width instead left the row wherever that
     share happened to land: a first card cut down the middle, a last one cut
     as well, and the next press cutting them somewhere else again. Where a
     card begins is read from the cards themselves rather than from the width
     they were asked to have, since the row rounds them and a rounding error
     repeated ten times along a row is a card out of place. */
  const move = (direction: 1 | -1) => {
    const element = track.current;
    if (!element) {
      return;
    }

    const cards = Array.from(element.querySelectorAll<HTMLElement>("[data-card]"));
    /* Where a card's left edge sits along the row, counted from the row's
       own beginning rather than from the screen: it does not change as the
       row moves, and it is the card's place rather than what is drawn, so a
       card grown under the pointer is still measured where it belongs. */
    const along = (card: HTMLElement) => card.offsetLeft - element.offsetLeft;

    if (cards.length < 2) {
      element.scrollBy({
        left: direction * element.clientWidth * ALMOST_A_SCREENFUL,
        behavior: "smooth",
      });
      return;
    }

    const first = along(cards[0]);
    const step = along(cards[1]) - first;
    if (step <= 0) {
      return;
    }

    /* The room kept at the sides for a card grown under the pointer. A card
       is in its place when it sits just inside that, which is where the
       heading above the row sits too. */
    const room = parseFloat(getComputedStyle(element).paddingLeft) || 0;
    const home = first - room;
    /* Which card the row is resting on, as a count of cards that can fall
       between two of them after a hand has pulled the row. */
    const resting = (element.scrollLeft - home) / step;
    /* Whole cards visible at once, and never less than one: a row wider than
       the window would otherwise answer a press by not moving. */
    const aScreenful = Math.max(1, Math.floor(element.clientWidth / step));
    const wanted =
      direction > 0
        ? Math.floor(resting + A_HAIR / step) + aScreenful
        : Math.ceil(resting - A_HAIR / step) - aScreenful;

    const landing = Math.min(Math.max(wanted, 0), cards.length - 1);
    element.scrollTo({ left: home + landing * step, behavior: "smooth" });
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
          <ChevronLeftIcon size={26} />
        </button>
      )}

      <div
        className="row-track"
        ref={track}
        onScroll={measure}
        onKeyDown={onKeyDown}
        {...drag}
      >
        {children}
      </div>

      {canGoOn && (
        <button
          className="row-arrow row-arrow-on"
          onClick={() => move(1)}
          aria-label={t("row.on")}
        >
          <ChevronRightIcon size={26} />
        </button>
      )}
    </div>
  );
}

/**
 * The heading of one row.
 *
 * A row that has a whole grid behind it says so twice: the heading itself
 * leads there, carrying the chevron that says it is a way through, and the
 * words at the right say it in words for whoever does not read chevrons. A
 * row that is only ever a row, like what somebody left halfway, has neither,
 * because a chevron leading nowhere is worse than no chevron.
 */
export function RowHead({
  mark,
  title,
  to,
  children,
}: {
  mark?: React.ReactNode;
  title: string;
  to?: string;
  children?: React.ReactNode;
}) {
  const { t } = useSettings();

  return (
    <div className="section-head">
      <h2>
        {mark}
        {to ? (
          <Link className="section-through" to={to}>
            {title}
            <ChevronRightIcon size={17} />
          </Link>
        ) : (
          title
        )}
      </h2>
      {children}
      {to && (
        <Link className="section-all" to={to}>
          {t("home.see_all")}
        </Link>
      )}
    </div>
  );
}
