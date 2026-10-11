/*
 * What every panel drawn over the page does the same way: the escape key
 * shuts it, the page underneath holds still, and the focus goes into it.
 */

import { useEffect, useRef } from "react";

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
    /* The page underneath holds still. Put back as it was rather than set to
       a value: another panel may have set it first, and two panels closing
       one after the other would otherwise leave the page unable to scroll. */
    const before = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    panel.current?.focus();
    return () => {
      document.removeEventListener("keydown", shut);
      document.body.style.overflow = before;
    };
  }, [panel]);
}
