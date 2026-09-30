/*
 * A screen of music drawn again whole when it is walked back to.
 *
 * What it last read is kept under a name, and its state begins from that
 * instead of from nothing: the page has its height from the first drawing,
 * so it can be put back where it was scrolled to, and it is read again quietly
 * to bring it up to date.
 */

import { useCallback, useState } from "react";
import { keep, recall } from "../kept";

/** State that begins from what was kept under this name, and keeps what it is
 *  set to. */
export function useKeptState<T>(name: string, empty: T): [T, (next: T) => void] {
  const [value, setValue] = useState<{ name: string; held: T }>(() => ({
    name,
    held: recall<T>(name)?.value ?? empty,
  }));
  const set = useCallback(
    (next: T) => {
      keep(name, next);
      setValue({ name, held: next });
    },
    [name],
  );
  // The same screen shown for another thing, an album for the next one, must
  // not be handed what the last one held.
  return [value.name === name ? value.held : (recall<T>(name)?.value ?? empty), set];
}
