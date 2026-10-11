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
 *
 * Every step into and out of the history is taken in turn, each waiting for
 * the one before to be over. One thing shutting while another opens, a panel
 * giving way to a window, would otherwise have the second entry put in before
 * the first was taken out, and the step back that was meant for the first
 * would land on the second.
 */

import { useCallback, useEffect, useRef } from "react";

const MARK = "melyxarOverlay";
/** How long to wait for the browser to say it went back, before going on. */
const GIVES_UP_MS = 300;

let counter = 0;
let queue: Promise<void> = Promise.resolve();

/** Runs a step once those before it are done. */
function inTurn(step: () => void | Promise<void>): void {
  queue = queue.then(step).catch(() => {});
}

interface Entry {
  id: number;
  /** Whether its entry is in the history, put there and not yet taken out. */
  live: boolean;
  /** Whether it is the one taking its entry out, which is no press of the back
   *  key. */
  popping: boolean;
}

function isTheLatest(entry: Entry): boolean {
  return window.history.state?.[MARK] === entry.id;
}

function putIn(entry: Entry): void {
  window.history.pushState({ ...window.history.state, [MARK]: entry.id }, "");
  entry.live = true;
}

/** Takes the entry out of the history, if it is still the last one in it. */
async function takeOut(entry: Entry): Promise<void> {
  if (!entry.live || !isTheLatest(entry)) {
    entry.live = false;
    return;
  }
  entry.live = false;
  entry.popping = true;
  await new Promise<void>((resolve) => {
    const done = () => {
      window.clearTimeout(timer);
      window.removeEventListener("popstate", done);
      entry.popping = false;
      resolve();
    };
    const timer = window.setTimeout(done, GIVES_UP_MS);
    window.addEventListener("popstate", done);
    window.history.back();
  });
}

/**
 * `onBack` is told when the back key was pressed, and answers true when what
 * it stands for is still there and the key should be waited for again: a
 * panel that went back to the one it came from. Nothing is done while
 * `enabled` is false.
 *
 * Returns what leaves the history for any other way of shutting: it takes the
 * entry out and then does what it is given, at once when there is none.
 */
export function useBackClosesIt(
  onBack: () => boolean | void,
  enabled = true,
): (then: () => void) => void {
  const mine = useRef<Entry | null>(null);
  const backRef = useRef(onBack);
  backRef.current = onBack;

  useEffect(() => {
    if (!enabled) {
      return;
    }
    const entry: Entry = { id: ++counter, live: false, popping: false };
    mine.current = entry;
    let wanted = true;
    const arm = () => {
      if (wanted) {
        putIn(entry);
      }
    };
    inTurn(arm);
    const moved = () => {
      if (!entry.live || entry.popping || isTheLatest(entry)) {
        return;
      }
      entry.live = false;
      if (backRef.current() === true) {
        inTurn(arm);
      }
    };
    window.addEventListener("popstate", moved);
    return () => {
      wanted = false;
      window.removeEventListener("popstate", moved);
      inTurn(() => takeOut(entry));
    };
  }, [enabled]);

  return useCallback((then: () => void) => {
    const entry = mine.current;
    if (!entry) {
      then();
      return;
    }
    inTurn(async () => {
      await takeOut(entry);
      then();
    });
  }, []);
}
