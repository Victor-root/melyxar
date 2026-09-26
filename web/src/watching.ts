/*
 * Where a viewer is in a work once they said whether they watched it.
 *
 * Marking a work watched lets go of where it was left: there is nothing to
 * carry on. Putting it back to unwatched keeps it: a film watched once and
 * started again is still where it was stopped, and saying it is not watched
 * after all moves nobody back to the beginning. The server holds the same
 * rules; these are what a screen shows before its answer comes back.
 */

import type { Seen } from "./api";

/** Where somebody is in a work: whether they watched it, and where to carry
 *  on from, in seconds, when there is anywhere. */
export interface Whereabouts {
  seen: Seen;
  resume: number | null;
}

/** Where a work stands once marked watched or not, from where it stood. */
export function markedWatched(watched: boolean, before: Whereabouts): Whereabouts {
  if (watched) {
    return { seen: "watched", resume: null };
  }
  return { seen: before.resume !== null ? "in_progress" : "not_started", resume: before.resume };
}

/** What was said about a work, and what the server had sent at the time. */
export interface Said {
  said: Whereabouts;
  over: Whereabouts;
}

/**
 * Where a work stands on a screen: what was said about it, while the server's
 * answer is still the one it was said over. A fresher answer is the server's
 * own account of what happened since, a film played to its end included, and
 * wins over anything said before it.
 */
export function whereaboutsOf(sent: Whereabouts, said: Said | undefined): Whereabouts {
  return said && said.over.seen === sent.seen && said.over.resume === sent.resume ? said.said : sent;
}
