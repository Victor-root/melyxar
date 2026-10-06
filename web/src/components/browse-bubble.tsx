/*
 * What a list is read by, on a phone: the sort, the filters and the ways to
 * put files in, in the bubble of the band at the top, rolled out from three
 * dots as the tabs of a library of music are.
 */

import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { useSettings } from "../settings";
import { BubbleParts, useBubble } from "./bubble";

export function BrowseBubble({ children }: { children: ReactNode }) {
  const { t } = useSettings();
  const { unrolled, unroll, stay, bubble } = useBubble();

  return createPortal(
    <nav
      ref={bubble}
      className="browse-piece music-tabs"
      aria-label={t("library.bar")}
      data-unrolled={unrolled ? "yes" : "no"}
      onPointerDown={unrolled ? stay : undefined}
    >
      <BubbleParts unrolled={unrolled} onUnroll={unroll} label={t("library.bar_open")} />
      <div className="music-tabs-track" onScroll={unrolled ? stay : undefined}>
        {children}
      </div>
    </nav>,
    document.body,
  );
}
