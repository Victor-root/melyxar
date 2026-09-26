/*
 * The tick of a card, and the bar that says how far in a film is.
 */

import { describe, expect, it } from "vitest";
import { markedWatched, whereaboutsOf } from "./watching";
import type { Whereabouts } from "./watching";

/** A film marked watched once, then started again and left a quarter in. */
const WATCHED_AND_STARTED_AGAIN: Whereabouts = { seen: "watched", resume: 1_800, unwatched: 0 };

describe("marking a work", () => {
  it("unwatched keeps where it was left", () => {
    expect(markedWatched(false, WATCHED_AND_STARTED_AGAIN, 0)).toEqual({
      seen: "in_progress",
      resume: 1_800,
      unwatched: 0,
    });
  });

  it("watched lets go of where it was left", () => {
    const unmarked = markedWatched(false, WATCHED_AND_STARTED_AGAIN, 0);
    expect(markedWatched(true, unmarked, 0)).toEqual({ seen: "watched", resume: null, unwatched: 0 });
  });

  it("unwatched with nowhere to carry on from is not started", () => {
    expect(markedWatched(false, { seen: "watched", resume: null, unwatched: 0 }, 0)).toEqual({
      seen: "not_started",
      resume: null,
      unwatched: 0,
    });
  });

  it("a series leaves every episode watched, or every one of them left", () => {
    const halfway: Whereabouts = { seen: "in_progress", resume: null, unwatched: 7 };
    expect(markedWatched(true, halfway, 12)).toEqual({ seen: "watched", resume: null, unwatched: 0 });
    expect(markedWatched(false, halfway, 12)).toEqual({
      seen: "not_started",
      resume: null,
      unwatched: 12,
    });
  });
});

describe("what a screen shows", () => {
  const said = {
    said: { seen: "in_progress" as const, resume: 1_800, unwatched: 0 },
    over: WATCHED_AND_STARTED_AGAIN,
  };

  it("is what was said while the server still sends what it was said over", () => {
    expect(whereaboutsOf(WATCHED_AND_STARTED_AGAIN, said)).toEqual(said.said);
  });

  it("is the server's answer once it has moved on", () => {
    const playedToTheEnd: Whereabouts = { seen: "watched", resume: null, unwatched: 0 };
    expect(whereaboutsOf(playedToTheEnd, said)).toEqual(playedToTheEnd);
  });

  it("is what the server sent when nothing was said", () => {
    expect(whereaboutsOf(WATCHED_AND_STARTED_AGAIN, undefined)).toEqual(WATCHED_AND_STARTED_AGAIN);
  });
});
