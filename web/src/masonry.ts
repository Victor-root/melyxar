/*
 * Cards of any height laid side by side and packed under one another, the way
 * notes are on a board, rather than in rows as tall as their tallest card.
 *
 * The page is a grid of fine rows, and each card spans as many of them as its
 * own height needs, measured whenever it changes. Where each goes is worked
 * out here rather than left to the grid: tallest first, so the longest cards
 * stand side by side from left to right, and each under the column that ends
 * highest, which is what fills the hole a
 * short card leaves under it with the next one. Columns ending about level
 * count as level, and then the leftmost wins, so the page reads left to right
 * rather than dropping a card in the middle for the sake of a few pixels.
 *
 * The order is the page's own until somebody uses it. A card that grows
 * because somebody opened something in it grows where it stands: laid out
 * tallest first again, it would jump to another column under the pointer that
 * just opened it. The order is worked out afresh when cards come or go, or
 * when the number of columns changes.
 */

import { useLayoutEffect } from "react";
import type { RefObject } from "react";

/** The height of one fine row, in pixels, as the stylesheet sets it. */
export const ROW_STEP = 1;

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
 * Where each card goes, answered in the order the cards were given.
 *
 * Between two wide cards, the ordinary ones are laid tallest first when there
 * is more than one column, those of
 * the same height in the order given; a wide card keeps its place, since it
 * says where one part of the page ends and the next begins. Each ordinary
 * card goes under the column ending highest, or under the leftmost of those
 * ending within `level` rows of it. A wide one starts below everything placed
 * so far, and everything after it starts below it.
 */
export function placed(
  cards: ToPlace[],
  columns: number,
  level: number,
  /** The order to lay them in, to keep the one they were laid in before. */
  kept?: number[],
): Placed[] {
  const ends: number[] = new Array(Math.max(1, columns)).fill(0);
  const where: Placed[] = new Array(cards.length);
  const order = kept ?? layingOrder(cards, ends.length);
  for (const index of order) {
    const card = cards[index];
    if (card.wide || ends.length === 1) {
      const row = Math.max(...ends);
      ends.fill(row + card.rows);
      where[index] = { column: 0, row };
      continue;
    }
    const highest = Math.min(...ends);
    const column = ends.findIndex((end) => end <= highest + level);
    const row = ends[column];
    ends[column] = row + card.rows;
    where[index] = { column, row };
  }
  return where;
}

/** The order cards are laid in when nobody has used the page: in a single
 *  column, where nothing stands side by side, the order they were written in;
 *  otherwise each run of ordinary
 *  cards tallest first, every wide card where it stands. */
export function layingOrder(cards: ToPlace[], columns: number): number[] {
  return columns <= 1 ? cards.map((_, index) => index) : tallestFirst(cards);
}

function tallestFirst(cards: ToPlace[]): number[] {
  const order: number[] = [];
  let run: number[] = [];
  const close = () => {
    order.push(...run.sort((one, two) => cards[two].rows - cards[one].rows));
    run = [];
  };
  cards.forEach((card, index) => {
    if (card.wide) {
      close();
      order.push(index);
    } else {
      run.push(index);
    }
  });
  close();
  return order;
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
    /** The cards in the order last laid, and over how many columns, once
     *  somebody has used the page; nothing until then. */
    let held: { cards: Element[]; columns: number } | null = null;
    let lastLaid: { cards: Element[]; columns: number } = { cards: [], columns: 0 };
    const lay = () => {
      const columns = getComputedStyle(holder)
        .gridTemplateColumns.split(" ")
        .filter(Boolean).length;
      const cards = [...holder.querySelectorAll<HTMLElement>(CARDS)];
      const wide = cards.map(isWide);
      const toPlace = cards.map((card, index) => ({ rows: rows.get(card) ?? 1, wide: wide[index] }));
      const stillHeld =
        held !== null &&
        held.columns === columns &&
        held.cards.length === cards.length &&
        cards.every((card) => held!.cards.includes(card));
      if (!stillHeld) {
        held = null;
      }
      const order = held
        ? held.cards.map((card) => cards.indexOf(card as HTMLElement))
        : layingOrder(toPlace, columns);
      lastLaid = { cards: order.map((index) => cards[index]), columns };
      const where = placed(toPlace, columns, Math.round(ABOUT_LEVEL / ROW_STEP), order);
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
        // A card may ask for no gap under it, as the title of a page does:
        // the side list and the first card start level with each other.
        const after = around.getPropertyValue("--board-gap-after").trim();
        rows.set(card, rowsFor(height, after === "" ? gap : parseFloat(after) || 0));
      }
      // Laid out again whole: one card growing moves every card after it,
      // and the page changing width changes how many columns there are.
      lay();
    });
    measured.observe(holder);
    // Somebody pressing or typing on the page is using it: from then on what
    // they open grows in place.
    const used = () => {
      held ??= lastLaid;
    };
    holder.addEventListener("pointerdown", used);
    holder.addEventListener("keydown", used);
    const follow = () => {
      const now = new Set(holder.querySelectorAll(CARDS));
      let gone = false;
      for (const card of rows.keys()) {
        if (!now.has(card)) {
          measured.unobserve(card);
          rows.delete(card);
          gone = true;
        }
      }
      for (const card of now) {
        if (!rows.has(card)) {
          rows.set(card, 1);
          measured.observe(card);
        }
      }
      // A card that arrives is measured, and laid out then. One that goes is
      // not measured again by anything, and the cards after it would keep the
      // places they had below it.
      if (gone) {
        lay();
      }
    };
    follow();
    const changes = new MutationObserver(follow);
    changes.observe(holder, { childList: true, subtree: true });
    return () => {
      holder.removeEventListener("pointerdown", used);
      holder.removeEventListener("keydown", used);
      changes.disconnect();
      measured.disconnect();
    };
  }, [page]);
}
