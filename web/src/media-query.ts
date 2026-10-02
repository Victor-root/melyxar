/*
 * Whether a media query holds, kept up to date as the window changes, for what
 * is drawn differently rather than only styled differently.
 */

import { useEffect, useState } from "react";

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
