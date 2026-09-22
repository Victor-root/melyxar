/*
 * The grid of cards, and the way a keyboard moves through it.
 *
 * Two things are deliberate here. The arrow keys move between cards the way
 * the eye does, which is what makes the interface usable from an armchair
 * rather than only from a desk. And the grid never asks how many rows there
 * are: it counts the columns from where the cards actually landed, so it stays
 * right at every width without a single measurement being hard coded.
 */

import { useCallback, useEffect, useRef } from "react";
import type { ReactNode } from "react";
import type { CardShape } from "./card";

interface GridProps {
  children: ReactNode;
  /** The shape of the cards it holds, which sets how wide a column is. */
  shape?: CardShape;
  /** Called when the viewer nears the end, to fetch what follows. */
  onReachEnd?: () => void;
  /** Whether there is anything left to fetch. */
  hasMore?: boolean;
}

export function Grid({ children, onReachEnd, hasMore, shape = "standing" }: GridProps) {
  const grid = useRef<HTMLDivElement>(null);
  const sentinel = useRef<HTMLDivElement>(null);

  // The next page is fetched when the end comes into view rather than when the
  // viewer hits the bottom, so the grid grows before it runs out.
  useEffect(() => {
    const target = sentinel.current;
    if (!target || !onReachEnd || !hasMore) {
      return;
    }
    const watcher = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          onReachEnd();
        }
      },
      { rootMargin: "600px" },
    );
    watcher.observe(target);
    return () => watcher.disconnect();
  }, [onReachEnd, hasMore]);

  const onKeyDown = useCallback((event: React.KeyboardEvent<HTMLDivElement>) => {
    const container = grid.current;
    if (!container) {
      return;
    }
    const cards = Array.from(
      container.querySelectorAll<HTMLElement>("[data-card]"),
    );
    const current = cards.indexOf(document.activeElement as HTMLElement);
    if (current < 0) {
      return;
    }

    const columns = countColumns(cards);
    let next: number | null = null;
    switch (event.key) {
      case "ArrowRight":
        next = current + 1;
        break;
      case "ArrowLeft":
        next = current - 1;
        break;
      case "ArrowDown":
        next = current + columns;
        break;
      case "ArrowUp":
        next = current - columns;
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

    if (next === null || next < 0 || next >= cards.length) {
      // Walking off the edge does nothing rather than wrapping around, which
      // would leave the eye somewhere it did not ask to be.
      return;
    }
    event.preventDefault();
    cards[next].focus();
    cards[next].scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, []);

  return (
    <>
      <div className={`grid grid-${shape}`} ref={grid} onKeyDown={onKeyDown}>
        {children}
      </div>
      <div ref={sentinel} aria-hidden="true" />
    </>
  );
}

/**
 * How many cards fit on a row, read from where they actually are.
 *
 * Counting the ones sharing the top edge of the first card is exact at every
 * width, in both directions of writing, and costs nothing.
 */
function countColumns(cards: HTMLElement[]): number {
  if (cards.length === 0) {
    return 1;
  }
  const firstTop = cards[0].offsetTop;
  const sameRow = cards.filter((card) => card.offsetTop === firstTop).length;
  return Math.max(1, sameRow);
}
