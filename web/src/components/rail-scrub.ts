/*
 * A rail of letters a finger can hold and slide along.
 *
 * A thumb on a phone is wider than a letter, and lifting it between two
 * letters is how a rail stops being usable. So a touch that lands on the rail
 * goes to the letter under it, and again to each letter it slides over, with
 * the list following. A mouse or a key still presses one letter at a time.
 *
 * A touch stays with the rail until the finger is lifted, wherever it goes:
 * the letter it is on is worked out from how far down it is.
 */

import { useRef } from "react";
import type { PointerEvent, RefObject } from "react";
import type { DropHandle } from "./letter-drop";

export function useRailScrub(go: (index: number) => void, drop: RefObject<DropHandle | null>) {
  const touching = useRef(false);
  const last = useRef(-1);

  /* Counted from where the first letter stands and how tall it is, so the
     finger may leave the rail, sideways or past either end, and the nearest
     letter is still the one it is on. */
  const follow = (event: PointerEvent<HTMLElement>) => {
    const letters = event.currentTarget.children;
    if (letters.length === 0) {
      return;
    }
    const first = letters[0].getBoundingClientRect();
    const reached = Math.floor((event.clientY - first.top) / first.height);
    const index = Math.min(Math.max(reached, 0), letters.length - 1);
    drop.current?.move(index, event.clientY);
    if (index !== last.current) {
      last.current = index;
      go(index);
    }
  };

  const release = () => {
    if (touching.current) {
      drop.current?.release();
    }
  };

  return {
    rail: {
      onPointerDown: (event: PointerEvent<HTMLElement>) => {
        touching.current = event.pointerType === "touch";
        last.current = -1;
        if (touching.current) {
          follow(event);
        }
      },
      onPointerMove: (event: PointerEvent<HTMLElement>) => {
        if (touching.current) {
          follow(event);
        }
      },
      onPointerUp: release,
      onPointerCancel: release,
    },
    /** What a press of one letter does: a touch has already gone there. */
    press: (index: number) => {
      if (!touching.current) {
        go(index);
      }
    },
  };
}
