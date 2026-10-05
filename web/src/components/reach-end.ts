/*
 * Runs something when the end of a list comes into view, a little before the
 * viewer reaches it, so the list grows before it runs out. Given nothing to
 * run, it watches nothing. The end is seen again each time what is run
 * changes, which is what keeps a list that is still short after a page
 * reading its next one.
 */

import { useEffect } from "react";
import type { RefObject } from "react";

export function useReachEnd(
  end: RefObject<Element | null>,
  onReach: (() => void) | null,
  margin: number,
): void {
  useEffect(() => {
    const target = end.current;
    if (!target || !onReach) {
      return;
    }
    const watcher = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          onReach();
        }
      },
      { rootMargin: `${margin}px` },
    );
    watcher.observe(target);
    return () => watcher.disconnect();
  }, [end, onReach, margin]);
}
