/*
 * Where a viewer is in a work once they said whether they watched it.
 *
 * Marking a work watched lets go of where it was left: there is nothing to
 * carry on. Putting it back to unwatched keeps it: a film watched once and
 * started again is still where it was stopped, and saying it is not watched
 * after all moves nobody back to the beginning. The server holds the same
 * rules; these are what a screen shows before its answer comes back.
 */

import type { Card, Seen } from "./api";

/** Where somebody is in a work: whether they watched it, where to carry on
 *  from, in seconds, when there is anywhere, and for a series or a season,
 *  how many of its episodes are left. */
export interface Whereabouts {
  seen: Seen;
  resume: number | null;
  unwatched: number;
}

/** Where a work stands once marked watched or not, from where it stood. A
 *  series or a season is marked through every episode under it, so all of
 *  them are left or none is. */
export function markedWatched(watched: boolean, before: Whereabouts, episodes: number): Whereabouts {
  if (watched) {
    return { seen: "watched", resume: null, unwatched: 0 };
  }
  return {
    seen: before.resume !== null ? "in_progress" : "not_started",
    resume: before.resume,
    unwatched: episodes,
  };
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
  return said &&
    said.over.seen === sent.seen &&
    said.over.resume === sent.resume &&
    said.over.unwatched === sent.unwatched
    ? said.said
    : sent;
}

/**
 * How long what a work is carried on in lasts, in seconds, which is what the
 * place it stopped is a share of: the copy's own length, and the one the
 * provider gives only when the copy was never measured. The provider's is
 * what a card says a work runs for, and a file holding two of its episodes
 * back to back runs twice as long: a place three quarters of the way into
 * that file drew as a full bar.
 */
export function lengthOfAPlay(
  card: Pick<Card, "resume_length_seconds" | "runtime_minutes">,
): number | null {
  if (card.resume_length_seconds !== null && card.resume_length_seconds > 0) {
    return card.resume_length_seconds;
  }
  return card.runtime_minutes && card.runtime_minutes > 0 ? card.runtime_minutes * 60 : null;
}
