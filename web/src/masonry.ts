/*
 * Cards of any height laid side by side and packed under one another, the way
 * notes are on a board, rather than in rows as tall as their tallest card.
 *
 * The page is a grid of fine rows, and each card spans as many of them as its
 * own height needs, measured whenever it changes. Where each goes is worked
 * out here rather than left to the grid: in the order the page gives them,
 * each under the column that ends highest, which is what fills the hole a
 * short card leaves under it with the next one. Columns ending about level
 * count as level, and then the leftmost wins, so the page reads left to right
 * rather than dropping a card in the middle for the sake of a few pixels.
 */

import { useLayoutEffect } from "react";
import type { RefObject } from "react";

/** The height of one fine row, in pixels, as the stylesheet sets it. */
export const ROW_STEP = 4;

/** How many fine rows a card of this height, margins included, spans, the
 *  gap under it included so the next card keeps its distance. */
export function rowsFor(height: number, gap: number, step: number = ROW_STEP): number {
  return Math.max(1, Math.ceil((height + gap) / step));
}

/** How far apart two columns may end and still count as level, in pixels. */
export const ABOUT_LEVEL = 100;

/** Where a card goes: its column, nought for the first, and the fine row it
 *  starts on, nought for the first. A wide card starts in column nought and
 *  spans them all. */
export interface Placed {
  column: number;
  row: number;
}

/** A card to place: how many fine rows it spans, and whether it takes the
 *  whole width. */
export interface ToPlace {
  rows: number;
  wide: boolean;
}

/**
 * Where each card goes, in the order given.
 *
 * An ordinary card goes under the column ending highest, or under the
 * leftmost of those ending within `level` rows of it. A wide one starts below
 * everything placed so far, and everything after it starts below it.
 */
export function placed(cards: ToPlace[], columns: number, level: number): Placed[] {
  const ends: number[] = new Array(Math.max(1, columns)).fill(0);
  return cards.map((card) => {
    if (card.wide || ends.length === 1) {
      const row = Math.max(...ends);
      ends.fill(row + card.rows);
      return { column: 0, row };
    }
    const highest = Math.min(...ends);
    const column = ends.findIndex((end) => end <= highest + level);
    const row = ends[column];
    ends[column] = row + card.rows;
    return { column, row };
  });
}

/** Every card of a page: what stands on the page itself, and the cards of a
 *  group of panels, which lay themselves on the page as if loose. */
const CARDS = ":scope > :not(.panels), :scope > .panels > *";

/**
 * Keeps every card of a page spanning the rows its height needs.
 *
 * Measured by an observer of sizes, which answers after layout and before
 * the page is painted, so nothing is ever drawn at a wrong height; and cards
 * that come and go as a page loads or changes are watched as they arrive.
 */
export function useMasonry(page: RefObject<HTMLElement | null>): void {
  useLayoutEffect(() => {
    const holder = page.current;
    if (!holder) {
      return;
    }
    const rows = new Map<Element, number>();
    /** Whether a card takes the whole width: anything standing on the page
     *  itself, and a panel of a group that asks for it. */
    const isWide = (card: HTMLElement) =>
      card.parentElement === holder || card.classList.contains("panel-wide");
    const lay = () => {
      const columns = getComputedStyle(holder)
        .gridTemplateColumns.split(" ")
        .filter(Boolean).length;
      const cards = [...holder.querySelectorAll<HTMLElement>(CARDS)];
      const wide = cards.map(isWide);
      const where = placed(
        cards.map((card, index) => ({ rows: rows.get(card) ?? 1, wide: wide[index] })),
        columns,
        Math.round(ABOUT_LEVEL / ROW_STEP),
      );
      cards.forEach((card, index) => {
        const { column, row } = where[index];
        card.style.gridColumn = wide[index] ? "1 / -1" : String(column + 1);
        card.style.gridRow = `${row + 1} / span ${rows.get(card) ?? 1}`;
      });
    };
    const measured = new ResizeObserver((entries) => {
      const gap = parseFloat(getComputedStyle(holder).columnGap) || 0;
      for (const entry of entries) {
        const card = entry.target as HTMLElement;
        if (card === holder) {
          continue;
        }
        // The margins count: the title of a page is lifted into the bar at
        // the top by a negative one, and what follows it starts where its
        // margin ends rather than where its box does.
        const around = getComputedStyle(card);
        const height =
          (entry.borderBoxSize[0]?.blockSize ?? card.offsetHeight) +
          (parseFloat(around.marginTop) || 0) +
          (parseFloat(around.marginBottom) || 0);
        rows.set(card, rowsFor(height, gap));
      }
      // Laid out again whole: one card growing moves every card after it,
      // and the page changing width changes how many columns there are.
      lay();
    });
    measured.observe(holder);
    const follow = () => {
      const now = new Set(holder.querySelectorAll(CARDS));
      for (const card of rows.keys()) {
        if (!now.has(card)) {
          measured.unobserve(card);
          rows.delete(card);
        }
      }
      for (const card of now) {
        if (!rows.has(card)) {
          rows.set(card, 1);
          measured.observe(card);
        }
      }
    };
    follow();
    const changes = new MutationObserver(follow);
    changes.observe(holder, { childList: true, subtree: true });
    return () => {
      changes.disconnect();
      measured.disconnect();
    };
  }, [page]);
}
