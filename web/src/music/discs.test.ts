import { describe, expect, it } from "vitest";
import type { Song } from "./api";
import { byDisc, minutesOf } from "./discs";

function song(title: string, disc: number | null, seconds: number | null = 200): Song {
  return {
    id: title,
    title,
    artists: [],
    album: null,
    track: 1,
    disc,
    year: null,
    seconds,
    source: null,
  };
}

describe("byDisc", () => {
  it("cuts an album on two discs into two", () => {
    const discs = byDisc([song("a", 1), song("b", 1), song("c", 2)]);
    expect(discs.map((disc) => [disc.number, disc.songs.map((one) => one.title)])).toEqual([
      [1, ["a", "b"]],
      [2, ["c"]],
    ]);
  });

  it("keeps an album on one disc whole and unnamed", () => {
    expect(byDisc([song("a", 1), song("b", 1)])).toEqual([
      { number: null, songs: [song("a", 1), song("b", 1)] },
    ]);
    expect(byDisc([song("a", null)])[0].number).toBeNull();
  });

  it("answers one empty disc for an album with no song", () => {
    expect(byDisc([])).toEqual([{ number: null, songs: [] }]);
  });
});

describe("minutesOf", () => {
  it("adds the songs up and rounds to the nearest minute", () => {
    expect(minutesOf([song("a", 1, 200), song("b", 1, 250)])).toBe(8);
    expect(minutesOf([song("a", 1, 29)])).toBe(0);
    expect(minutesOf([song("a", 1, 30)])).toBe(1);
  });

  it("counts a song whose length is unknown as nothing", () => {
    expect(minutesOf([song("a", 1, null), song("b", 1, 120)])).toBe(2);
  });
});
