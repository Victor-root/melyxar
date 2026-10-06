/*
 * Whether something that has just been taken off the screen is still on it,
 * for the time its way out takes: it stays drawn for `ms` after it should be
 * gone, so it can leave rather than vanish. Nothing lingers where there is
 * nothing to show, or for no time at all.
 *
 * Decided before the screen is painted, so the one frame between its going
 * and its leaving is never drawn empty.
 */

import { useEffect, useState } from "react";

export function useLeaving(shown: boolean, canLinger: boolean, ms: number): boolean {
  /* Noted while drawing rather than after, so the drawing where it is taken
     off already says it is leaving: noted after, it was taken off the page
     for one drawing and made again whole for the next. */
  const [was, setWas] = useState(shown);
  const [leaving, setLeaving] = useState(false);
  if (was !== shown) {
    setWas(shown);
    setLeaving(!shown && canLinger && ms > 0);
  } else if (leaving && !canLinger) {
    setLeaving(false);
  }
  useEffect(() => {
    if (!leaving) {
      return;
    }
    const timer = setTimeout(() => setLeaving(false), ms);
    return () => clearTimeout(timer);
  }, [leaving, ms]);
  return leaving;
}
