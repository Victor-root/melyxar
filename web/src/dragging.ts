/*
 * Running a sideways row by holding it and pulling.
 *
 * A finger already does this on a touchscreen and two fingers already do it on
 * a trackpad. A mouse is the one hand left with nothing but the arrows at each
 * end, and a row of twenty four cards reached two screens at a time by
 * pressing a small button is a row nobody reaches the end of.
 *
 * Written once and used by every sideways row there is, the player's own
 * included: the two behaved differently, and the one that behaved well was the
 * one nobody could reuse.
 *
 * Touch and pen are left alone. Both already scroll such a row natively, and a
 * second hand doing the same thing at the same time fights the first.
 */

import { useRef } from "react";
import type { RefObject } from "react";

/** How far the hand moves before a press becomes a drag. */
const A_PRESS_THAT_MOVED = 4;

/** What to spread onto the element that scrolls. */
export interface DragToScroll {
  onPointerDown: (event: React.PointerEvent) => void;
  onPointerMove: (event: React.PointerEvent) => void;
  onPointerUp: (event: React.PointerEvent) => void;
  onPointerCancel: (event: React.PointerEvent) => void;
  onClickCapture: (event: React.MouseEvent) => void;
}

export function useDragToScroll(row: RefObject<HTMLElement | null>): DragToScroll {
  const from = useRef<{ x: number; scrollLeft: number } | null>(null);
  const moved = useRef(false);

  return {
    onPointerDown: (event) => {
      const element = row.current;
      if (event.pointerType !== "mouse" || event.button !== 0 || !element) {
        return;
      }
      from.current = { x: event.clientX, scrollLeft: element.scrollLeft };
      // Not captured yet: a plain press that never moves is a press on
      // whatever card is under it, and capturing the pointer here would carry
      // the click that ends it away to this row instead of to that card,
      // whether or not a drag ever happened.
    },

    onPointerMove: (event) => {
      const start = from.current;
      const element = row.current;
      if (!start || !element) {
        return;
      }
      const by = event.clientX - start.x;
      // Capturing only now, the moment a press stops being a press, is what
      // leaves an ordinary click alone while still following the hand
      // wherever it goes once a drag is really under way.
      if (!moved.current && Math.abs(by) > A_PRESS_THAT_MOVED) {
        moved.current = true;
        element.setPointerCapture(event.pointerId);
      }
      element.scrollLeft = start.scrollLeft - by;
    },

    onPointerUp: (event) => {
      if (!from.current) {
        return;
      }
      from.current = null;
      if (row.current?.hasPointerCapture(event.pointerId)) {
        row.current.releasePointerCapture(event.pointerId);
      }
    },

    onPointerCancel: (event) => {
      if (!from.current) {
        return;
      }
      from.current = null;
      if (row.current?.hasPointerCapture(event.pointerId)) {
        row.current.releasePointerCapture(event.pointerId);
      }
    },

    /* A drag that really moved the row still ends in a click, on whatever card
       the hand happens to be over: caught here, in the one place above every
       card, rather than taught to each kind of card a row might ever hold. */
    onClickCapture: (event) => {
      if (moved.current) {
        moved.current = false;
        event.preventDefault();
        event.stopPropagation();
      }
    },
  };
}
