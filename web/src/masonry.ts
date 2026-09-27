/*
 * Cards of any height laid side by side and packed under one another, the way
 * notes are on a board, rather than in rows as tall as their tallest card.
 *
 * The page is a grid of fine rows, and each card spans as many of them as its
 * own height needs, measured whenever it changes. The grid then places each
 * card in the first place it fits, in the order the page gives them, which is
 * what fills the hole a short card leaves under it with the next one.
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
    let gap = 0;
    const measured = new ResizeObserver((entries) => {
      gap = parseFloat(getComputedStyle(holder).columnGap) || 0;
      for (const entry of entries) {
        const card = entry.target as HTMLElement;
        // The margins count: the title of a page is lifted into the bar at
        // the top by a negative one, and what follows it starts where its
        // margin ends rather than where its box does.
        const around = getComputedStyle(card);
        const height =
          (entry.borderBoxSize[0]?.blockSize ?? card.offsetHeight) +
          (parseFloat(around.marginTop) || 0) +
          (parseFloat(around.marginBottom) || 0);
        card.style.gridRowEnd = `span ${rowsFor(height, gap)}`;
      }
    });
    const watched = new Set<Element>();
    const follow = () => {
      const now = new Set(holder.querySelectorAll(CARDS));
      for (const card of watched) {
        if (!now.has(card)) {
          measured.unobserve(card);
          watched.delete(card);
        }
      }
      for (const card of now) {
        if (!watched.has(card)) {
          measured.observe(card);
          watched.add(card);
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
