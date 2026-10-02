/*
 * Whether something that has just been taken off the screen is still on it,
 * for the time its way out takes: it stays drawn for `ms` after it should be
 * gone, so it can leave rather than vanish. Nothing lingers where there is
 * nothing to show, or for no time at all.
 *
 * Decided before the screen is painted, so the one frame between its going
 * and its leaving is never drawn empty.
 */

import { useLayoutEffect, useRef, useState } from "react";

export function useLeaving(shown: boolean, canLinger: boolean, ms: number): boolean {
  const [leaving, setLeaving] = useState(false);
  const wasShown = useRef(false);
  useLayoutEffect(() => {
    if (shown) {
      wasShown.current = true;
      setLeaving(false);
      return;
    }
    if (!wasShown.current || !canLinger || ms === 0) {
      wasShown.current = false;
      setLeaving(false);
      return;
    }
    setLeaving(true);
    const timer = setTimeout(() => {
      wasShown.current = false;
      setLeaving(false);
    }, ms);
    return () => clearTimeout(timer);
  }, [shown, canLinger, ms]);
  return leaving;
}
