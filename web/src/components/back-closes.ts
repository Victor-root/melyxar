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
 *
 * An entry can still be left behind: a link pressed in a menu goes to its
 * page before the menu has shut, and the menu's entry is then under the new
 * page rather than on top. Gone back to later, such an entry stands for
 * nothing, so the step back goes straight on past it rather than seeming to
 * do nothing. And whatever goes back a page, or replaces the page, while
 * something is open over it asks for that once everything open has left the
 * history (see `afterTheOverlays`).
 */

import { useCallback, useEffect, useRef } from "react";

const MARK = "melyxarOverlay";
/** How long to wait for the browser to say it went back, before going on. */
const GIVES_UP_MS = 300;

/** Told apart from the entries of an earlier load of the page, which are in
 *  the history still and stand for nothing any more. */
const THIS_LOAD = Math.random().toString(36).slice(2);
let counter = 0;
let queue: Promise<void> = Promise.resolve();
/** The entries in the history now, oldest first. */
const live: Entry[] = [];
/** The entries asked to be put in and not put in yet, first asked first. */
const waiting: Entry[] = [];

/** Runs a step once those before it are done. */
function inTurn(step: () => void | Promise<void>): void {
  queue = queue.then(step).catch(() => {});
}

interface Entry {
  id: string;
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
  live.push(entry);
}

function leftOut(entry: Entry): void {
  entry.live = false;
  const at = live.indexOf(entry);
  if (at >= 0) {
    live.splice(at, 1);
  }
}

/** Whether the history stands on the entry of something no longer open. */
function onALeftEntry(): boolean {
  const landed: unknown = window.history.state?.[MARK];
  return typeof landed === "string" && !live.some((entry) => entry.id === landed);
}

/** Whether the history stands on the entry or on one put in after it and
 *  still there: the step back shut only what was open over it, a list over
 *  a window's step. */
function standsOnOrAbove(entry: Entry): boolean {
  const landed: unknown = window.history.state?.[MARK];
  return live.slice(live.indexOf(entry)).some((it) => it.id === landed);
}

/* Gone back to such an entry, the step back goes on past it. */
if (typeof window !== "undefined") {
  window.addEventListener("popstate", () => {
    if (onALeftEntry()) {
      window.history.back();
    }
  });
}

/**
 * Gives the entry's place in the history to the next one waiting to be put
 * in, rather than going back from it and adding another: one thing giving
 * way to another, a menu to the window one of its lines opens, is then one
 * entry that changes hands. Gone back and added again, the second entry
 * would be one added without the hand of whoever pressed, which a phone's
 * back key steps over, leaving the application instead of shutting it.
 */
function handOver(entry: Entry): boolean {
  const next = waiting.shift();
  if (!next) {
    return false;
  }
  window.history.replaceState({ ...window.history.state, [MARK]: next.id }, "");
  leftOut(entry);
  next.live = true;
  live.push(next);
  return true;
}

/** Takes the entry out of the history, if it is still the last one in it,
 *  or hands its place over when `mayHandOver` and something waits for one. */
async function takeOut(entry: Entry, mayHandOver = true): Promise<void> {
  if (!entry.live || !isTheLatest(entry)) {
    leftOut(entry);
    return;
  }
  if (mayHandOver && handOver(entry)) {
    return;
  }
  leftOut(entry);
  entry.popping = true;
  await new Promise<void>((resolve) => {
    const done = () => {
      window.clearTimeout(timer);
      window.removeEventListener("popstate", landed);
      entry.popping = false;
      resolve();
    };
    // Landed on an entry left behind, the step back is still going on.
    const landed = () => {
      if (!onALeftEntry()) {
        done();
      }
    };
    const timer = window.setTimeout(done, GIVES_UP_MS);
    window.addEventListener("popstate", landed);
    window.history.back();
  });
}

/**
 * `onBack` is told when the back key was pressed, and answers true when what
 * it stands for is still there and the key should be waited for again: a
 * panel that went back to the one it came from. Nothing is done while
 * `enabled` is false. A new `key` while it is enabled is something new in the
 * same place, the next episode in the player: a step of its own.
 *
 * Returns what leaves the history for any other way of shutting: it takes the
 * entry out and then does what it is given, at once when there is none.
 */
export function useBackClosesIt(
  onBack: () => boolean | void,
  enabled = true,
  key: string | null = null,
): (then: () => void) => void {
  const mine = useRef<Entry | null>(null);
  const backRef = useRef(onBack);
  backRef.current = onBack;

  useEffect(() => {
    if (!enabled) {
      return;
    }
    counter += 1;
    const entry: Entry = { id: `${THIS_LOAD}:${counter}`, live: false, popping: false };
    mine.current = entry;
    let wanted = true;
    const arm = () => {
      const stillWaiting = waiting.indexOf(entry);
      if (stillWaiting >= 0) {
        waiting.splice(stillWaiting, 1);
      }
      if (wanted && !entry.live) {
        putIn(entry);
      }
    };
    waiting.push(entry);
    inTurn(arm);
    const moved = () => {
      if (!entry.live || entry.popping || standsOnOrAbove(entry)) {
        return;
      }
      leftOut(entry);
      if (backRef.current() === true) {
        inTurn(arm);
      }
    };
    window.addEventListener("popstate", moved);
    return () => {
      wanted = false;
      const stillWaiting = waiting.indexOf(entry);
      if (stillWaiting >= 0) {
        waiting.splice(stillWaiting, 1);
      }
      window.removeEventListener("popstate", moved);
      inTurn(() => takeOut(entry));
    };
  }, [enabled, key]);

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

/**
 * Does what it is given once everything open over the page has left the
 * history: for going back a page, or replacing it, from inside something
 * open over it, which would otherwise go back from the open thing or replace
 * its entry instead of the page's.
 */
export function afterTheOverlays(then: () => void): void {
  inTurn(async () => {
    for (const entry of [...live].reverse()) {
      await takeOut(entry, false);
    }
    then();
  });
}
