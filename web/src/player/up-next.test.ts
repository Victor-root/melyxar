import { describe, expect, it } from "vitest";
import type { NextEpisode, PlaybackSegment } from "../api";
import { shareLeft, upNextOffered } from "./up-next";

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
    expect(upNextOffered(NEXT, false, false, CREDITS, 1259, 1320)).toBe(false);
    expect(upNextOffered(NEXT, false, false, CREDITS, 1260, 1320)).toBe(true);
  });

  it("waits for the credits when they are known, however near the end", () => {
    expect(upNextOffered(NEXT, false, false, [{ ...CREDITS[0], from_second: 1310 }], 1300, 1320)).toBe(false);
  });

  it("is offered over the last seconds when the credits are not known", () => {
    expect(upNextOffered(NEXT, false, false, [], 1289, 1320)).toBe(false);
    expect(upNextOffered(NEXT, false, false, [], 1290, 1320)).toBe(true);
    expect(upNextOffered(NEXT, false, true, [], 1321, 1320)).toBe(true);
  });

  it("is not offered before the film says how long it is", () => {
    expect(upNextOffered(NEXT, false, false, [], 0, 0)).toBe(false);
  });

  it("is not offered again once turned down, nor when there is none", () => {
    expect(upNextOffered(NEXT, true, true, CREDITS, 1300, 1320)).toBe(false);
    expect(upNextOffered(null, false, true, CREDITS, 1300, 1320)).toBe(false);
  });
});

describe("the ring of the count", () => {
  it("is whole at the start, half at half time and gone at the end", () => {
    expect(shareLeft(37, 37)).toBe(1);
    expect(shareLeft(18.5, 37)).toBe(0.5);
    expect(shareLeft(0, 37)).toBe(0);
    expect(shareLeft(-1, 37)).toBe(0);
    expect(shareLeft(0, 0)).toBe(0);
  });
});
