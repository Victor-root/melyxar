/*
 * The tabs of a library of music, in one piece of the glass of the browse
 * bar. When they do not all fit, they are a carousel: an arrow at the edge
 * that has more beyond it moves along, the tab open is always brought into
 * view, and a hand can pull them too.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { useDragToScroll } from "../dragging";
import { ChevronLeftIcon, ChevronRightIcon } from "../icons";
import { useSettings } from "../settings";
import type { MusicTab } from "./tabs";

/** How much of the width one press of an arrow moves. */
const ALMOST_A_SCREENFUL = 0.8;

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

  useEffect(() => {
    track.current
      ?.querySelector<HTMLElement>("[aria-current]")
      ?.scrollIntoView({ inline: "nearest", block: "nearest" });
  }, [open]);

  const move = (direction: 1 | -1) => {
    const element = track.current;
    if (element) {
      element.scrollBy({ left: direction * element.clientWidth * ALMOST_A_SCREENFUL, behavior: "smooth" });
    }
  };

  return (
    <nav className="browse-piece music-tabs" aria-label={t("music.tabs")}>
      {canGoBack && (
        <button type="button" className="music-tabs-arrow" onClick={() => move(-1)} aria-label={t("music.tabs_back")}>
          <ChevronLeftIcon size={18} />
        </button>
      )}
      <div className="music-tabs-track" ref={track} onScroll={measure} {...drag}>
        {tabs.map((tab) => (
          <button
            key={tab}
            type="button"
            className={`music-tab${tab === open ? " music-tab-on" : ""}`}
            aria-current={tab === open ? "page" : undefined}
            onClick={() => onOpen(tab)}
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
}
