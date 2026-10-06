/*
 * A rail of letters a finger can hold and slide along.
 *
 * A thumb on a phone is wider than a letter, and lifting it between two
 * letters is how a rail stops being usable. So a touch that lands on the rail
 * goes to the letter under it, and again to each letter it slides over, with
 * the list following. A mouse or a key still presses one letter at a time.
 *
 * Which letter is under the finger is asked of the screen at the finger's
 * position: a touch stays with the letter it landed on, so the one it has
 * reached is never the one that receives the event.
 */

import { useRef } from "react";
import type { PointerEvent } from "react";

/** Put on each letter of the rail, with its place along it. */
export const LETTER_ATTRIBUTE = "data-letter";

export function useRailScrub(go: (index: number) => void) {
  const touching = useRef(false);
  const last = useRef(-1);

  const follow = (event: PointerEvent) => {
    const under = document.elementFromPoint(event.clientX, event.clientY);
    const letter = under?.closest<HTMLElement>(`[${LETTER_ATTRIBUTE}]`);
    if (!letter) {
      return;
    }
    const index = Number(letter.getAttribute(LETTER_ATTRIBUTE));
    if (index !== last.current) {
      last.current = index;
      go(index);
    }
  };

  return {
    rail: {
      onPointerDown: (event: PointerEvent) => {
        touching.current = event.pointerType === "touch";
        last.current = -1;
        if (touching.current) {
          follow(event);
        }
      },
      onPointerMove: (event: PointerEvent) => {
        if (touching.current) {
          follow(event);
        }
      },
    },
    /** What a press of one letter does: a touch has already gone there. */
    press: (index: number) => {
      if (!touching.current) {
        go(index);
      }
    },
  };
}
