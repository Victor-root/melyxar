import { describe, expect, it } from "vitest";
import type { Found, TitleRequest } from "./api";
import { standingOf } from "./standing";

const found = (changes: Partial<Found> = {}): Found => ({
  catalogue: "films",
  tmdb_id: "1",
  title: "Amber Field",
  original_title: null,
  year: 2020,
  overview: null,
  poster: null,
  held: null,
  asked_by: 0,
  mine: null,
  ...changes,
});

const request = (changes: Partial<TitleRequest> = {}): TitleRequest => ({
  id: "r",
  user_name: "Sam",
  catalogue: "films",
  tmdb_id: "1",
  title: "Amber Field",
  year: 2020,
  poster: null,
  overview: null,
  seasons: [],
  note: "",
  state: "pending",
  answer: "",
  work_id: null,
  created_at: "2026-10-04T12:00:00Z",
  decided_at: null,
  ...changes,
});

describe("where an answer stands", () => {
  it("is free when nobody asked and nothing is here", () => {
    expect(standingOf(found(), [])).toEqual({ is: "free", others: 0, heldSeasons: [] });
  });

  it("is here for a film held, opening it", () => {
    expect(standingOf(found({ held: { work_id: "w", seasons: [] } }), [])).toEqual({ is: "here", workId: "w" });
  });

  it("stays free for a series held in part, naming what it holds", () => {
    const series = found({ catalogue: "series", held: { work_id: "w", seasons: [1, 2] } });
    expect(standingOf(series, [])).toEqual({ is: "free", others: 0, heldSeasons: [1, 2] });
  });

  it("follows this account's requests as they are now, not as the search found them", () => {
    const asked = found({ asked_by: 3, mine: "r" });
    expect(standingOf(asked, [request()])).toMatchObject({ is: "mine", others: 2 });
    expect(standingOf(asked, [])).toEqual({ is: "free", others: 2, heldSeasons: [] });
    expect(standingOf(asked, [request({ state: "refused" })])).toMatchObject({ is: "free" });
    expect(standingOf(found({ asked_by: 1 }), [request()])).toMatchObject({ is: "mine", others: 1 });
  });

  it("does not take a series for the film sharing its number", () => {
    expect(standingOf(found({ catalogue: "series" }), [request()])).toMatchObject({ is: "free" });
  });
});
