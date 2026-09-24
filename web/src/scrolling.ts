/*
 * Where somebody was on a page, given back when they come back to it.
 *
 * The library scrolls in a box of its own rather than in the window, so the
 * browser, which keeps the place of a page it scrolled itself, keeps nothing
 * here: going back landed at the top of every page. So the place is kept by
 * hand, once for the whole interface. Every page of the history has its own:
 * how far down the box was, and how far along each of its rows.
 *
 * Coming back is not one jump. The page asks the server again before it has
 * anything to show, and a grid holds only its first cards until it is
 * scrolled; the place is asked for again on every frame while the page grows
 * into it, until it has been reached and the page has held still a moment,
 * or somebody takes the page in hand. A new page starts at the top, and a
 * page that only rewrote its own address stays where it is.
 */

import { useEffect, useLayoutEffect, useRef } from "react";
import { useLocation, useNavigationType } from "react-router-dom";

/** Where one page was left. */
export interface Place {
  /** How far down the box was scrolled. */
  top: number;
  /** How far along each row of the page was, in the order they stand. */
  rows: number[];
}

/** How many pages of the history keep their place: a long evening of going
 *  back and forth, and never a list that grows for ever. */
const PAGES_KEPT = 60;

/** How long a page is given to grow back into its place. */
const GIVING_UP_MS = 8000;

/** How long a page has to hold still once the place is reached, so a page
 *  that is still arriving does not shift under it afterwards. */
const SETTLED_MS = 400;

/** A pixel of slack: a scroll position is rarely a whole number. */
const A_PIXEL = 1;

/** Where the places are kept across a reload of the page, for as long as the
 *  tab lives. */
const STORED = "melyxar.places";

/** Keeps one more place, the most recent last, forgetting the oldest past the
 *  number kept. */
export function remember(
  places: Map<string, Place>,
  page: string,
  place: Place,
  most: number,
): Map<string, Place> {
  const kept = new Map(places);
  kept.delete(page);
  kept.set(page, place);
  while (kept.size > most) {
    const oldest = kept.keys().next().value;
    if (oldest === undefined) {
      break;
    }
    kept.delete(oldest);
  }
  return kept;
}

/** Whether the page stands where it was left: the box, and every row it
 *  holds that has grown long enough to be put back. */
export function isThere(now: Place, wanted: Place): boolean {
  return (
    Math.abs(now.top - wanted.top) <= A_PIXEL &&
    wanted.rows.every(
      (along, index) => now.rows[index] !== undefined && Math.abs(now.rows[index] - along) <= A_PIXEL,
    )
  );
}

function rowsOf(box: HTMLElement): HTMLElement[] {
  return Array.from(box.querySelectorAll<HTMLElement>(".row-track"));
}

function placeOf(box: HTMLElement): Place {
  return { top: box.scrollTop, rows: rowsOf(box).map((row) => row.scrollLeft) };
}

function readStored(): Map<string, Place> {
  try {
    const stored = sessionStorage.getItem(STORED);
    return new Map(stored ? (JSON.parse(stored) as [string, Place][]) : []);
  } catch {
    // A tab that keeps nothing starts with nothing kept.
    return new Map();
  }
}

function store(places: Map<string, Place>) {
  try {
    sessionStorage.setItem(STORED, JSON.stringify(Array.from(places.entries())));
  } catch {
    // Kept in memory all the same, which is what going back within the tab
    // needs; only a reload forgets.
  }
}

export function useKeptPlaces(scroller: React.RefObject<HTMLElement | null>) {
  const location = useLocation();
  const how = useNavigationType();
  const page = useRef(location.key);
  const places = useRef<Map<string, Place>>(readStored());

  /* Written down as the page moves: the box and every row in it, since a
     row's own scrolling is caught on its way through the box. Once a frame,
     however many times it moved in it. */
  useEffect(() => {
    const box = scroller.current;
    if (!box) {
      return;
    }
    let frame = 0;
    const note = () => {
      frame = 0;
      places.current = remember(places.current, page.current, placeOf(box), PAGES_KEPT);
    };
    const moved = () => {
      if (frame === 0) {
        frame = requestAnimationFrame(note);
      }
    };
    const leaving = () => store(places.current);
    box.addEventListener("scroll", moved, { capture: true, passive: true });
    window.addEventListener("pagehide", leaving);
    return () => {
      cancelAnimationFrame(frame);
      box.removeEventListener("scroll", moved, { capture: true });
      window.removeEventListener("pagehide", leaving);
    };
  }, [scroller]);

  useLayoutEffect(() => {
    page.current = location.key;
    const box = scroller.current;
    if (!box) {
      return;
    }
    if (how === "PUSH") {
      box.scrollTop = 0;
      return;
    }
    const wanted = places.current.get(location.key);
    if (how !== "POP" || !wanted) {
      return;
    }

    let frame = 0;
    const began = performance.now();
    let tall = -1;
    let stillSince = began;
    const stop = () => {
      cancelAnimationFrame(frame);
      for (const hand of HANDS) {
        box.removeEventListener(hand, stop);
      }
    };
    const step = () => {
      const now = performance.now();
      box.scrollTop = wanted.top;
      rowsOf(box).forEach((row, index) => {
        if (wanted.rows[index] !== undefined) {
          row.scrollLeft = wanted.rows[index];
        }
      });
      if (box.scrollHeight !== tall) {
        tall = box.scrollHeight;
        stillSince = now;
      }
      const settled = isThere(placeOf(box), wanted) && now - stillSince > SETTLED_MS;
      if (settled || now - began > GIVING_UP_MS) {
        stop();
        return;
      }
      frame = requestAnimationFrame(step);
    };
    /* Somebody reaching for the page wins over the place being put back. */
    for (const hand of HANDS) {
      box.addEventListener(hand, stop, { passive: true });
    }
    step();
    return stop;
  }, [location.key, how, scroller]);
}

/** What a hand on the page looks like, any one of which ends the putting
 *  back. */
const HANDS = ["wheel", "touchstart", "pointerdown", "keydown"] as const;
