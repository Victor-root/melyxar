/*
 * The tabs of a library of music as pages on a phone: a dot for each at the
 * foot of the screen, the open one lit, as the banner of the home page shows
 * where it is in its five, and a swipe sideways over a tab to reach the one
 * beside it.
 *
 * A swipe that starts over something that scrolls sideways, and can still
 * scroll that way, is that thing's: the pages of what was listened to, a row
 * of covers. Once it has reached its end, the next swipe turns the tab.
 */

import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { useSettings } from "../settings";
import { swipeOf } from "./tabs";
import type { MusicTab } from "./tabs";

export function TabDots({ tabs, open, onOpen }: { tabs: MusicTab[]; open: MusicTab; onOpen: (tab: MusicTab) => void }) {
  const { t } = useSettings();
  // Drawn over the page rather than in it: the foot of the page fades into
  // the player, and the dots would fade with it.
  return createPortal(
    <nav className="music-tab-dots" aria-label={t("music.tabs")}>
      {tabs.map((tab) => (
        <button
          key={tab}
          type="button"
          className={`hero-dot${tab === open ? " hero-dot-on" : ""}`}
          aria-label={t(`music.tab.${tab}`)}
          aria-current={tab === open ? "page" : undefined}
          onClick={() => onOpen(tab)}
        />
      ))}
    </nav>,
    document.body,
  );
}

/** Calls `onSwipe` with the way a finger swiped across `area`, when it did. */
export function useTabSwipe(
  area: React.RefObject<HTMLElement | null>,
  on: boolean,
  onSwipe: (way: -1 | 1) => void,
) {
  const latest = useRef(onSwipe);
  latest.current = onSwipe;
  useEffect(() => {
    const element = area.current;
    if (!on || !element) {
      return;
    }
    let start: { x: number; y: number; target: EventTarget | null } | null = null;
    const down = (event: TouchEvent) => {
      const touch = event.touches[0];
      start = event.touches.length === 1 ? { x: touch.clientX, y: touch.clientY, target: event.target } : null;
    };
    const up = (event: TouchEvent) => {
      const touch = event.changedTouches[0];
      if (!start || !touch) {
        return;
      }
      const way = swipeOf(touch.clientX - start.x, touch.clientY - start.y);
      const from = start.target;
      start = null;
      if (way !== 0 && !takenOnTheWay(from, element, way)) {
        latest.current(way);
      }
    };
    element.addEventListener("touchstart", down, { passive: true });
    element.addEventListener("touchend", up, { passive: true });
    return () => {
      element.removeEventListener("touchstart", down);
      element.removeEventListener("touchend", up);
    };
  }, [area, on]);
}

/** Whether something between where the finger began and the tab scrolls
 *  sideways and could still go that way, or is a field or a slider. */
function takenOnTheWay(from: EventTarget | null, area: HTMLElement, way: -1 | 1): boolean {
  for (let at = from instanceof Element ? from : null; at && at !== area; at = at.parentElement) {
    if (at instanceof HTMLInputElement || at instanceof HTMLTextAreaElement) {
      return true;
    }
    if (at.scrollWidth > at.clientWidth + 1 && /auto|scroll/.test(getComputedStyle(at).overflowX)) {
      const room = way === 1 ? at.scrollWidth - at.clientWidth - at.scrollLeft : at.scrollLeft;
      if (room > 1) {
        return true;
      }
    }
  }
  return false;
}
