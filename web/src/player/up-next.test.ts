import { describe, expect, it } from "vitest";
import type { NextEpisode, PlaybackSegment } from "../api";
import { COUNT_FROM, shareLeft, upNextOffered } from "./up-next";

const NEXT: NextEpisode = {
  id: "e2",
  season: 1,
  episode: 2,
  title: "A Second Night",
  source_id: "s2",
  overview: null,
  still: [],
};
const CREDITS = [{ kind: "outro", from_second: 1260, to_second: 1320 }] as PlaybackSegment[];

describe("the next episode", () => {
  it("is offered from the start of the credits", () => {
    expect(upNextOffered(NEXT, false, false, CREDITS, 1259)).toBe(false);
    expect(upNextOffered(NEXT, false, false, CREDITS, 1260)).toBe(true);
  });

  it("is offered at the very end when the credits are not known", () => {
    expect(upNextOffered(NEXT, false, false, [], 1300)).toBe(false);
    expect(upNextOffered(NEXT, false, true, [], 1321)).toBe(true);
  });

  it("is not offered again once turned down, nor when there is none", () => {
    expect(upNextOffered(NEXT, true, true, CREDITS, 1300)).toBe(false);
    expect(upNextOffered(null, false, true, CREDITS, 1300)).toBe(false);
  });
});

describe("the ring of the count", () => {
  it("is whole at the start, half at half time and gone at the end", () => {
    expect(shareLeft(COUNT_FROM)).toBe(1);
    expect(shareLeft(COUNT_FROM / 2)).toBe(0.5);
    expect(shareLeft(0)).toBe(0);
    expect(shareLeft(-1)).toBe(0);
  });
});
