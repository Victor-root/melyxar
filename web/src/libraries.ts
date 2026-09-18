/*
 * The libraries the whole interface is built from, held in one place.
 *
 * The navigation, the home page, the search and the activity screen are all
 * drawn from this one list, and the settings screen is where it changes.
 * Fetched once per screen, a library declared or taken away would leave the
 * bar at the top listing something that is no longer there until somebody
 * reloaded the page, which is exactly what this interface promises never to
 * make anyone do.
 *
 * So it is read here, and read again whenever something changed it: at once
 * when this interface changed it, and whenever work ends, since a scan is what
 * makes a library grow.
 */

import { createContext, useCallback, useContext, useEffect, useState } from "react";
import { api } from "./api";
import type { Library } from "./api";

export interface Libraries {
  all: Library[];
  /** Told to look again now, after declaring one or taking one away. */
  refresh: () => void;
}

export const LibrariesContext = createContext<Libraries>({
  all: [],
  refresh: () => {},
});

/**
 * What the provider at the top of the interface holds.
 *
 * Reads again each time work ends, which is how the count beside a library
 * grows during a scan without anybody asking.
 */
export function useWatchedLibraries(finished: number): Libraries {
  const [all, setAll] = useState<Library[]>([]);
  const [asked, setAsked] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    api
      .libraries(controller.signal)
      .then(setAll)
      // A server that did not answer keeps the list that was there: emptying
      // the navigation over one failed request would say the collection is
      // gone, which is a far worse thing to say than nothing.
      .catch(() => {});
    return () => controller.abort();
  }, [finished, asked]);

  const refresh = useCallback(() => setAsked((count) => count + 1), []);

  return { all, refresh };
}

/** The libraries, for any page or part of the bar that shows them. */
export function useLibraries(): Libraries {
  return useContext(LibrariesContext);
}
