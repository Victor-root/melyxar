/*
 * A box that takes the rest of the screen below where it begins, so what it
 * holds scrolls inside it and the page itself has nothing to scroll.
 */

import { useLayoutEffect } from "react";
import { scrollerOf } from "../landing";

/** How tall the box is: what is left of the screen under its top once the
 *  foot of the page is kept, and never less than `least`, on a screen too
 *  short for that, where the page scrolls after all. */
export function roomBelow(viewport: number, top: number, reserve: number, least: number): number {
  return Math.max(least, viewport - top - reserve);
}

/** `peek` is left under the box on top of the foot of the page, for the
 *  first of what comes after it to show, so the page is seen to go on. */
export function useRoomBelow(box: React.RefObject<HTMLElement | null>, least: number, peek = 0) {
  useLayoutEffect(() => {
    const element = box.current;
    const scroller = element && scrollerOf(element);
    if (!element || !scroller) {
      return;
    }
    const page = element.closest(".page") ?? scroller;
    const measure = () => {
      const foot = parseFloat(getComputedStyle(page).paddingBottom) || 0;
      const top = element.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop;
      element.style.height = `${roomBelow(scroller.clientHeight, top, foot + peek, least)}px`;
    };
    measure();
    const watcher = new ResizeObserver(measure);
    watcher.observe(scroller);
    if (element.parentElement) {
      watcher.observe(element.parentElement);
    }
    return () => watcher.disconnect();
  }, [box, least, peek]);
}
