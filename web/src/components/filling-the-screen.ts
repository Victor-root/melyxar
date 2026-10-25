/*
 * What fills the screen right now, if anything. The browser draws nothing
 * else while something does, so whatever has to stay in sight over a filled
 * screen (a word in the corner, the journal, the frame counter) is drawn
 * inside it.
 */

import { useEffect, useState } from "react";

export function useFillingTheScreen(): Element | null {
  const [filling, setFilling] = useState<Element | null>(() => document.fullscreenElement);
  useEffect(() => {
    const follow = () => setFilling(document.fullscreenElement);
    document.addEventListener("fullscreenchange", follow);
    return () => document.removeEventListener("fullscreenchange", follow);
  }, []);
  return filling;
}
