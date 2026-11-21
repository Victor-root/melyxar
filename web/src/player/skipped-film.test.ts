import { describe, expect, it } from "vitest";
import { filmLeftOutAhead, forgetThrownAway, whereItLies, type Stretch } from "./skipped-film";

const SEAM = 0.5;

/** Where the segments of a film cut on its key frames lie, some very short. */
const lies: Record<number, Stretch> = {
  10: { start: 100, end: 101 },
  11: { start: 101, end: 101.3 },
  12: { start: 101.3, end: 101.385 },
  13: { start: 101.385, end: 101.47 },
  14: { start: 101.47, end: 101.555 },
  15: { start: 101.555, end: 102.5 },
  16: { start: 102.5, end: 103.5 },
};

function fetchedOf(...numbers: number[]): Map<number, Stretch> {
  return new Map(numbers.map((sn) => [sn, lies[sn]]));
}

const withAHole = [
  { start: 100, end: 101.3 },
  { start: 101.555, end: 103.5 },
];

describe("film the library left out", () => {
  it("finds segments never fetched inside a gap narrower than a seam", () => {
    expect(filmLeftOutAhead(withAHole, 100.5, fetchedOf(10, 11, 15, 16), SEAM)).toEqual({ from: 101.3, after: 11 });
  });

  it("finds it as well with the film standing right at the gap", () => {
    expect(filmLeftOutAhead(withAHole, 101.3, fetchedOf(10, 11, 15, 16), SEAM)).toEqual({ from: 101.3, after: 11 });
  });

  it("leaves a seam between two readings alone", () => {
    // The sound of the second reading lands a little after its picture.
    const held = [
      { start: 100, end: 101.3 },
      { start: 101.32, end: 103.5 },
    ];
    expect(filmLeftOutAhead(held, 100.5, fetchedOf(10, 11, 12, 13, 14, 15, 16), SEAM)).toBeNull();
  });

  it("leaves a gap wider than a seam to the library", () => {
    const held = [
      { start: 100, end: 101 },
      { start: 102.5, end: 103.5 },
    ];
    expect(filmLeftOutAhead(held, 100.5, fetchedOf(10, 16), SEAM)).toBeNull();
  });

  it("says nothing with the film outside what is held", () => {
    expect(filmLeftOutAhead(withAHole, 99, fetchedOf(10, 11, 15, 16), SEAM)).toBeNull();
  });

  it("says nothing with nothing held past the film", () => {
    expect(filmLeftOutAhead([{ start: 100, end: 101.3 }], 100.5, fetchedOf(10, 11), SEAM)).toBeNull();
  });

  it("says nothing when the segments on either side are not known", () => {
    expect(filmLeftOutAhead(withAHole, 100.5, fetchedOf(10, 11), SEAM)).toBeNull();
  });
});

describe("where a handed segment lies", () => {
  it("is where the browser placed its picture, not where the playlist put it", () => {
    const handed = {
      start: 90.09,
      end: 90.59,
      elementaryStreams: { video: { startPTS: 90.674, endPTS: 90.757 }, audio: { startPTS: 90.6, endPTS: 90.7 } },
    };
    expect(whereItLies(handed)).toEqual({ start: 90.674, end: 90.757 });
  });

  it("is where the playlist put it when the browser said nothing", () => {
    expect(whereItLies({ start: 4, end: 8, elementaryStreams: {} })).toEqual({ start: 4, end: 8 });
  });
});

describe("film thrown away", () => {
  it("forgets the segments it held and keeps the others", () => {
    const fetched = fetchedOf(10, 11, 15, 16);
    forgetThrownAway(fetched, 101.3, Number.POSITIVE_INFINITY);
    expect([...fetched.keys()]).toEqual([10, 11]);
  });

  it("forgets everything when the whole film is thrown away", () => {
    const fetched = fetchedOf(10, 11, 15, 16);
    forgetThrownAway(fetched, 0, Number.POSITIVE_INFINITY);
    expect(fetched.size).toBe(0);
  });
});
