/*
 * What every panel drawn over the page does the same way: the escape key
 * shuts it, the page underneath holds still, and the focus goes into it.
 */

import { useEffect, useRef } from "react";

/** How many panels hold the page still right now, and what its scrolling was
 *  before the first of them. Counted rather than put back as each one found
 *  it: a sheet that is still sliding away when the panel it opened arrives
 *  would otherwise unlock the page under that panel. */
let holding = 0;
let scrollingBefore = "";

export function useHoldsTheScreen(panel: React.RefObject<HTMLElement | null>, onClose: () => void) {
  /* Read at the moment of use: a parent that draws again, at every letter
     typed in a field, hands a new function each time, and an effect that
     depended on it would take the focus back to the panel at every one. */
  const shutRef = useRef(onClose);
  shutRef.current = onClose;

  useEffect(() => {
    const shut = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        shutRef.current();
      }
    };
    document.addEventListener("keydown", shut);
    /* The page underneath holds still until the last panel is gone. */
    if (holding === 0) {
      scrollingBefore = document.body.style.overflow;
      document.body.style.overflow = "hidden";
    }
    holding += 1;
    panel.current?.focus();
    return () => {
      document.removeEventListener("keydown", shut);
      holding -= 1;
      if (holding === 0) {
        document.body.style.overflow = scrollingBefore;
      }
    };
  }, [panel]);
}
