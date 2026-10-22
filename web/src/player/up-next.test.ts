import { describe, expect, it } from "vitest";
import type { NextEpisode, PlaybackSegment } from "../api";
import { upNextOffered } from "./up-next";

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
