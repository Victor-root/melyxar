/*
 * Whether a media query holds, kept up to date as the window changes, for what
 * is drawn differently rather than only styled differently.
 */

import { useEffect, useState } from "react";

/** A phone, which is where the style sheet for narrow screens takes over. */
export const PHONE = "(max-width: 600px)";

/** A screen touched with a finger, of any size. */
export const COARSE = "(pointer: coarse)";

/** A phone held upright, which is where the player gathers its less used
 *  buttons into a menu. */
export const PHONE_UPRIGHT = "(max-width: 720px) and (orientation: portrait)";

/** A phone held sideways, which is short rather than narrow. */
export const PHONE_SIDEWAYS = "(max-height: 500px) and (orientation: landscape)";

/** Where the title page stacks its poster, facts and actions in one column. */
export const STACKED = "(max-width: 800px)";

export function useMediaQuery(query: string): boolean {
  const [holds, setHolds] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    const watcher = window.matchMedia(query);
    const change = () => setHolds(watcher.matches);
    change();
    watcher.addEventListener("change", change);
    return () => watcher.removeEventListener("change", change);
  }, [query]);
  return holds;
}
