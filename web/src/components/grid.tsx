/*
 * The grid of cards, and the way a keyboard moves through it.
 *
 * Two things are deliberate here. The arrow keys move between cards the way
 * the eye does, which is what makes the interface usable from an armchair
 * rather than only from a desk. And the grid never asks how many rows there
 * are: it reads how many columns the stylesheet made at the width it is drawn
 * at, so it stays right at every width without a single measurement being
 * hard coded.
 *
 * The cards are held in blocks of a few whole rows, each one a grid of its
 * own laid out the same way, which the browser skips while it is off the
 * screen. Held in one grid, every card that came into view had the whole grid
 * laid out again, thousands of cards of it for a large library: measured on
 * three thousand films slowed four times, a fifth of a second for every frame
 * of a scroll, and a tenth of that once in blocks. A block is cut from whole
 * rows, so the rows read on from one block to the next as they did in one
 * grid.
 */

import { Children, useCallback, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { ReactNode, RefObject } from "react";
import { flushSync } from "react-dom";
import { useFetchingAhead } from "./card";
import type { CardShape } from "./card";
import { useReachEnd } from "./reach-end";

/** How many rows of cards a block holds: about a screen of them. */
const ROWS_A_BLOCK = 3;

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
  const columns = useColumns(grid, shape);

  /* Everything the grid holds must draw something: a block cut short by a
     card that drew nothing would end on a row with a hole in it. */
  const items = useMemo(() => Children.toArray(children), [children]);
  const blocks = useMemo(() => {
    if (columns === null) {
      return [];
    }
    const size = columns * ROWS_A_BLOCK;
    const cut: ReactNode[] = [];
    for (let start = 0; start < items.length; start += size) {
      const held = items.slice(start, start + size);
      cut.push(
        <div
          key={start / size}
          className={`grid grid-${shape} grid-block`}
          style={{ ["--rows" as string]: Math.ceil(held.length / columns) }}
        >
          {held}
        </div>,
      );
    }
    return cut;
  }, [items, columns, shape]);

  useFetchingAhead(grid);

  // The next page is fetched when the end comes into view rather than when the
  // viewer hits the bottom, so the grid grows before it runs out.
  useReachEnd(sentinel, hasMore ? (onReachEnd ?? null) : null, 600);

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

    let next: number | null = null;
    switch (event.key) {
      case "ArrowRight":
        next = current + 1;
        break;
      case "ArrowLeft":
        next = current - 1;
        break;
      case "ArrowDown":
        next = current + (columns ?? 1);
        break;
      case "ArrowUp":
        next = current - (columns ?? 1);
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
  }, [columns]);

  /* One box holding both, so the end is always under the last card. Left
     loose beside it, the end became a neighbour of whatever the grid was set
     beside, the letters, and sat at the top of the page: always in view, it
     either fetched the whole library at once or never fetched again. */
  return (
    <div>
      <div className={`grid grid-${shape}`} ref={grid} onKeyDown={onKeyDown}>
        {blocks}
      </div>
      <div ref={sentinel} aria-hidden="true" />
    </div>
  );
}

/** The columns last read for each shape of card, so a grid opened again is
 *  cut into blocks of the right size from its first drawing. A grid of a shape
 *  never read yet holds no card until its columns are read, still before the
 *  frame is drawn: cut by a guess, every card was made twice, once in the
 *  blocks of the guess and again in the right ones. */
const counted = new Map<CardShape, number>();

/**
 * How many cards fit on a row, as the stylesheet laid the grid out at the
 * width it has: the tracks it made, read back. Read again whenever the grid
 * changes width, and put in place before the frame is drawn, so the blocks
 * never show a row cut in two.
 */
function useColumns(grid: RefObject<HTMLDivElement | null>, shape: CardShape): number | null {
  const [columns, setColumns] = useState(() => counted.get(shape) ?? null);
  useLayoutEffect(() => {
    const element = grid.current;
    if (!element) {
      return;
    }
    const read = () => {
      const tracks = getComputedStyle(element)
        .gridTemplateColumns.split(" ")
        .filter((track) => track.endsWith("px")).length;
      if (tracks > 0) {
        counted.set(shape, tracks);
        setColumns(tracks);
      }
    };
    read();
    const watcher = new ResizeObserver(() => flushSync(read));
    watcher.observe(element);
    return () => watcher.disconnect();
  }, [grid, shape]);
  return columns;
}
