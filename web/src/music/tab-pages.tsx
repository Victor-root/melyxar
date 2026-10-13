/*
 * The tabs of a library of music as pages on a phone: a dot for each just
 * under the band at the top, the open one lit, as the banner of the home page
 * shows where it is in its five, between two chevrons that go to the tab
 * before and the one after.
 */

import { ChevronLeftIcon, ChevronRightIcon } from "../icons";
import { useSettings } from "../settings";
import { tabBeside } from "./tabs";
import type { MusicTab } from "./tabs";

export function TabDots({ tabs, open, onOpen }: { tabs: MusicTab[]; open: MusicTab; onOpen: (tab: MusicTab) => void }) {
  const { t } = useSettings();
  const before = tabBeside(tabs, open, -1);
  const after = tabBeside(tabs, open, 1);
  return (
    <nav className="music-tab-dots" aria-label={t("music.tabs")}>
      <button
        type="button"
        className="music-tab-step"
        disabled={!before}
        aria-label={before ? t(`music.tab.${before}`) : undefined}
        onClick={() => before && onOpen(before)}
      >
        <ChevronLeftIcon size={18} />
      </button>
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
      <button
        type="button"
        className="music-tab-step"
        disabled={!after}
        aria-label={after ? t(`music.tab.${after}`) : undefined}
        onClick={() => after && onOpen(after)}
      >
        <ChevronRightIcon size={18} />
      </button>
    </nav>
  );
}
