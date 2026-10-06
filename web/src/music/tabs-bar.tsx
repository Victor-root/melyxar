/*
 * The tabs of a library of music, in one piece of the glass of the browse
 * bar. When they do not all fit, they are a carousel: an arrow at the edge
 * that has more beyond it moves along, the tab open is always brought into
 * view, and a hand can pull them too.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useDragToScroll } from "../dragging";
import { ChevronLeftIcon, ChevronRightIcon } from "../icons";
import { PHONE, useMediaQuery } from "../media-query";
import { useSettings } from "../settings";
import type { MusicTab } from "./tabs";

/** The room the arrow at the start takes once it is there, its width and the
 *  gap beside it as the stylesheet draws them: it appears as the tabs move,
 *  and pushes them along by as much. */
const ARROW_ROOM = 32;

/** How long the bubble takes to roll out, as the stylesheet draws it. */
const UNROLL_MS = 340;

/** How long the tabs of a phone stay rolled out once nobody has touched them. */
const UNROLLED_MS = 2000;

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
  const [unrolled, setUnrolled] = useState(false);
  const rollUp = useRef(0);

  const stayUnrolled = useCallback(() => {
    window.clearTimeout(rollUp.current);
    rollUp.current = window.setTimeout(() => setUnrolled(false), UNROLLED_MS);
  }, []);

  useEffect(() => () => window.clearTimeout(rollUp.current), []);

  /* A press anywhere else rolls them up at once. */
  const bubble = useRef<HTMLElement>(null);
  useEffect(() => {
    if (!unrolled) {
      return;
    }
    const away = (event: PointerEvent) => {
      if (!bubble.current?.contains(event.target as Node)) {
        setUnrolled(false);
      }
    };
    document.addEventListener("pointerdown", away, true);
    return () => document.removeEventListener("pointerdown", away, true);
  }, [unrolled]);

  const measure = useCallback(() => {
    const element = track.current;
    if (!element) {
      return;
    }
    setCanGoBack(element.scrollLeft > 1);
    setCanGoOn(element.scrollLeft + element.clientWidth < element.scrollWidth - 1);
  }, []);

  useEffect(() => {
    const element = track.current;
    if (!element) {
      return;
    }
    measure();
    const watcher = new ResizeObserver(measure);
    watcher.observe(element);
    for (const child of Array.from(element.children)) {
      watcher.observe(child);
    }
    return () => watcher.disconnect();
  }, [measure, tabs]);

  /* Rolled up, the tabs of a phone have a bubble's width to be seen in, and
     bringing the open one into it would leave them pulled along when they
     roll out. They start from their beginning then, the open one brought in
     only if the width they end with does not show it. */
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
    const later = window.setTimeout(
      () => current?.scrollIntoView({ inline: "nearest", block: "nearest", behavior: "smooth" }),
      UNROLL_MS,
    );
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
      onPointerDown={unrolled ? stayUnrolled : undefined}
    >
      {phone && (
        <>
          <span className="music-tabs-glass music-tabs-glass-start" aria-hidden="true">
            <span className="glass-slice" />
          </span>
          <span className="music-tabs-glass music-tabs-glass-end" aria-hidden="true">
            <span className="glass-slice" />
          </span>
        </>
      )}
      {phone && (
        <button
          type="button"
          className="music-bubble"
          aria-label={t("music.tabs_open")}
          aria-expanded={unrolled}
          onClick={() => {
            setUnrolled(true);
            stayUnrolled();
          }}
        >
          <svg viewBox="0 0 24 24" width="22" height="22" fill="currentColor" aria-hidden="true">
            <circle cx="5" cy="12" r="2.6" />
            <circle cx="12" cy="12" r="2.6" />
            <circle cx="19" cy="12" r="2.6" />
          </svg>
        </button>
      )}
      {canGoBack && (
        <button type="button" className="music-tabs-arrow" onClick={() => move(-1)} aria-label={t("music.tabs_back")}>
          <ChevronLeftIcon size={18} />
        </button>
      )}
      <div
        className="music-tabs-track"
        ref={track}
        onScroll={() => {
          measure();
          if (unrolled) {
            stayUnrolled();
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
