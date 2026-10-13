/*
 * The tabs of a library of music, in one piece of the glass of the browse
 * bar. When they do not all fit, they are a carousel: an arrow at the edge
 * that has more beyond it moves along, the tab open is always brought into
 * view, and a hand can pull them too.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { BubbleParts, UNROLL_MS, useBubble } from "../components/bubble";
import { useDragToScroll } from "../dragging";
import { ChevronLeftIcon, ChevronRightIcon } from "../icons";
import { PHONE, useMediaQuery } from "../media-query";
import { useSettings } from "../settings";
import type { MusicTab } from "./tabs";

/** The room the arrow at the start takes once it is there, its width and the
 *  gap beside it as the stylesheet draws them: it appears as the tabs move,
 *  and pushes them along by as much. */
const ARROW_ROOM = 32;

/** The room kept at either end of the tabs of a phone, as the stylesheet
 *  draws it. */
const TRACK_PADDING = 5;

export function TabsBar({
  tabs,
  open,
  onOpen,
}: {
  tabs: MusicTab[];
  open: MusicTab;
  onOpen: (tab: MusicTab) => void;
}) {
  const { t } = useSettings();
  const track = useRef<HTMLDivElement>(null);
  const [canGoBack, setCanGoBack] = useState(false);
  const [canGoOn, setCanGoOn] = useState(false);
  const drag = useDragToScroll(track);
  /* On a phone the tabs are a small bubble in the band at the top, rolled out
     by a press and rolled up again by themselves. */
  const phone = useMediaQuery(PHONE);
  const { unrolled, setUnrolled, unroll, stay, bubble } = useBubble();

  const measure = useCallback(() => {
    const element = track.current;
    if (!element) {
      return;
    }
    setCanGoBack(element.scrollLeft > 1);
    setCanGoOn(element.scrollLeft + element.clientWidth < element.scrollWidth - 1);
  }, []);

  /* The arrows are for a wide screen only: a phone pulls its tabs by hand,
     and reading where they stand at every step of that was work for
     nothing. */
  useEffect(() => {
    const element = track.current;
    if (!element || phone) {
      return;
    }
    // Its first reading comes once the page is laid out, not by laying it out.
    const watcher = new ResizeObserver(measure);
    watcher.observe(element);
    for (const child of Array.from(element.children)) {
      watcher.observe(child);
    }
    return () => watcher.disconnect();
  }, [measure, tabs, phone]);

  /* Rolled up, the tabs of a phone have a bubble's width to be seen in, and
     bringing the open one into it would leave them pulled along when they
     roll out. They start from their beginning then, the open one brought in
     only if the width they end with does not show it. Where it stands is read
     as the bubble starts to roll, the page already laid out for it, and only
     the slide is left for its end: asked then, where to bring it had the
     page laid out again on the frame the bubble finished on. */
  useEffect(() => {
    const element = track.current;
    if (!element || (phone && !unrolled)) {
      return;
    }
    const current = element.querySelector<HTMLElement>("[aria-current]");
    if (!phone) {
      current?.scrollIntoView({ inline: "nearest", block: "nearest" });
      return;
    }
    element.scrollLeft = 0;
    if (!current) {
      return;
    }
    const beyond = current.offsetLeft + current.offsetWidth + TRACK_PADDING - element.clientWidth;
    if (beyond <= 0) {
      return;
    }
    const later = window.setTimeout(() => element.scrollTo({ left: beyond, behavior: "smooth" }), UNROLL_MS);
    return () => window.clearTimeout(later);
  }, [open, phone, unrolled]);

  /* One tab at a time: the next one cut off at the edge is brought in whole
     and nothing else is asked of it. */
  const move = (direction: 1 | -1) => {
    const element = track.current;
    if (!element) {
      return;
    }
    const edge = element.getBoundingClientRect();
    const tabs = Array.from(element.children);
    if (direction === 1) {
      const next = tabs.find((tab) => tab.getBoundingClientRect().right > edge.right + 1);
      if (next) {
        const arrow = canGoBack ? 0 : ARROW_ROOM;
        element.scrollBy({ left: next.getBoundingClientRect().right - edge.right + arrow, behavior: "smooth" });
      }
    } else {
      const before = tabs.reverse().find((tab) => tab.getBoundingClientRect().left < edge.left - 1);
      if (before) {
        element.scrollBy({ left: before.getBoundingClientRect().left - edge.left, behavior: "smooth" });
      }
    }
  };

  const bar = (
    <nav
      ref={bubble}
      className="browse-piece music-tabs"
      aria-label={t("music.tabs")}
      data-unrolled={unrolled ? "yes" : "no"}
      onPointerDown={unrolled ? stay : undefined}
    >
      {phone && <BubbleParts unrolled={unrolled} onUnroll={unroll} label={t("music.tabs_open")} />}
      {canGoBack && (
        <button type="button" className="music-tabs-arrow" onClick={() => move(-1)} aria-label={t("music.tabs_back")}>
          <ChevronLeftIcon size={18} />
        </button>
      )}
      <div
        className="music-tabs-track"
        ref={track}
        onScroll={() => {
          if (!phone) {
            measure();
          }
          if (unrolled) {
            stay();
          }
        }}
        {...drag}
      >
        {tabs.map((tab) => (
          <button
            key={tab}
            type="button"
            className={`music-tab${tab === open ? " music-tab-on" : ""}`}
            aria-current={tab === open ? "page" : undefined}
            onClick={() => {
              setUnrolled(false);
              onOpen(tab);
            }}
          >
            {t(`music.tab.${tab}`)}
          </button>
        ))}
      </div>
      {canGoOn && (
        <button type="button" className="music-tabs-arrow" onClick={() => move(1)} aria-label={t("music.tabs_on")}>
          <ChevronRightIcon size={18} />
        </button>
      )}
    </nav>
  );

  return phone ? createPortal(bar, document.body) : bar;
}
