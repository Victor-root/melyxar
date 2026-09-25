import { describe, expect, it } from "vitest";
import { HITCHES_NAMED, readTheOpening, type Presented } from "./opening";

/** A film of `rate` pictures a second, shown one after the other on time. */
function smooth(count: number, rate: number, from = 0): Presented[] {
  const picture = 1000 / rate;
  return Array.from({ length: count }, (_, index) => ({
    shownAt: from + index * picture,
    presented: index + 1,
    media: (index * picture) / 1000,
    afterABreak: false,
  }));
}

describe("readTheOpening", () => {
  it("finds nothing in a film that flowed", () => {
    const opening = readTheOpening(smooth(240, 24), [], []);
    expect(opening.picture_ms).toBeCloseTo(41.7, 1);
    expect(opening.held + opening.skipped + opening.blind).toBe(0);
    expect(opening.first_hitches).toEqual([]);
  });

  it("does not mistake the uneven cadence of a film on a faster screen for a hitch", () => {
    // Twenty four pictures a second on a screen of sixty: two refreshes, then
    // three, then two.
    const pictures = smooth(48, 24).map((picture, index) => ({
      ...picture,
      shownAt: Math.floor(index / 2) * 83.3 + (index % 2) * 33.3,
    }));
    expect(readTheOpening(pictures, [], []).held).toBe(0);
  });

  it("names a picture held on the screen too long, and what the page was doing then", () => {
    const pictures = smooth(100, 24);
    for (let index = 50; index < pictures.length; index += 1) {
      pictures[index].shownAt += 120;
    }
    const opening = readTheOpening(pictures, [{ startedAt: 2050, lasted: 80 }], [
      { at: 2000, clock: 2.0 },
      { at: 2210, clock: 2.05 },
    ]);
    expect(opening.held).toBe(1);
    expect(opening.worst_held_ms).toBe(162);
    expect(opening.first_hitches).toEqual([
      {
        kind: "held",
        at_ms: 2042,
        at_second: 2.083,
        gap_ms: 162,
        pictures_lost: 0,
        busy_ms: 80,
        clock_ms: 50,
      },
    ]);
  });

  it("counts pictures of the film that were never shown", () => {
    const pictures = smooth(100, 25).filter((_, index) => index < 40 || index > 42);
    for (let index = 40; index < pictures.length; index += 1) {
      pictures[index].presented -= 3;
    }
    const opening = readTheOpening(pictures, [], []);
    expect(opening.skipped).toBe(1);
    expect(opening.pictures_lost).toBe(3);
    expect(opening.held).toBe(0);
  });

  it("tells a page too busy to look apart from a picture that stood still", () => {
    // The browser went on showing pictures; this page was told of none of them.
    const pictures = smooth(100, 30).filter((_, index) => index < 30 || index > 35);
    const opening = readTheOpening(pictures, [], []);
    expect(opening.blind).toBe(1);
    expect(opening.held + opening.skipped).toBe(0);
  });

  it("reads nothing into the time a film spent paused or moved", () => {
    const pictures = [...smooth(50, 24), ...smooth(50, 24, 10_000)];
    pictures[50] = { ...pictures[50], presented: 51, afterABreak: true };
    for (let index = 51; index < pictures.length; index += 1) {
      pictures[index].presented = index + 1;
    }
    expect(readTheOpening(pictures, [], []).first_hitches).toEqual([]);
  });

  it("names the first few hitches and only counts the rest", () => {
    const pictures = smooth(400, 24);
    for (let index = 10; index < pictures.length; index += 1) {
      pictures[index].shownAt += Math.floor((index - 10) / 20 + 1) * 100;
    }
    const opening = readTheOpening(pictures, [], []);
    expect(opening.held).toBeGreaterThan(HITCHES_NAMED);
    expect(opening.first_hitches).toHaveLength(HITCHES_NAMED);
  });
});
