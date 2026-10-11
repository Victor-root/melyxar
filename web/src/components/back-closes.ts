/*
 * The back key of a phone shuts what is drawn over the page instead of going
 * back a page under it.
 *
 * Something that opens puts one entry of its own into the history, at the
 * same address, so that going back has an entry to land on that is not a
 * page. The router's own state is carried over into it, which is what makes
 * the router read the step back as no move at all. Shut any other way, the
 * entry is taken out again, so that the history is left as it was found: and
 * taken out before whatever the press was for is done, since a page opened
 * from here would otherwise be opened above an entry that is no longer
 * needed, and the way back from it would stop there once for nothing.
 */

import { useCallback, useEffect, useRef } from "react";

const MARK = "melyxarOverlay";
/** How long to wait for the browser to say it went back, before going on. */
const GIVES_UP_MS = 300;

let counter = 0;

/**
 * `onBack` is told when the back key was pressed. Returns what leaves the
 * history for any other way of shutting: it takes the entry out and then does
 * what it is given, at once when there is no entry to take out.
 */
export function useBackClosesIt(onBack: () => void): (then: () => void) => void {
  const id = useRef(0);
  const live = useRef(false);
  const waiting = useRef<(() => void) | null>(null);
  const backRef = useRef(onBack);
  backRef.current = onBack;

  useEffect(() => {
    id.current = ++counter;
    live.current = true;
    window.history.pushState({ ...window.history.state, [MARK]: id.current }, "");
    const moved = () => {
      if (window.history.state?.[MARK] === id.current) {
        return;
      }
      const done = waiting.current;
      waiting.current = null;
      if (!live.current) {
        return;
      }
      live.current = false;
      if (done) {
        done();
      } else {
        backRef.current();
      }
    };
    window.addEventListener("popstate", moved);
    return () => {
      window.removeEventListener("popstate", moved);
      if (live.current && window.history.state?.[MARK] === id.current) {
        live.current = false;
        window.history.back();
      }
    };
  }, []);

  return useCallback((then: () => void) => {
    if (!live.current || window.history.state?.[MARK] !== id.current) {
      live.current = false;
      then();
      return;
    }
    const timer = window.setTimeout(() => {
      const done = waiting.current;
      waiting.current = null;
      live.current = false;
      done?.();
    }, GIVES_UP_MS);
    waiting.current = () => {
      window.clearTimeout(timer);
      then();
    };
    window.history.back();
  }, []);
}
