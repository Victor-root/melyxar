import { describe, expect, it } from "vitest";

import { FLAT_PATH, TRACK_LENGTH, WAVY_PATH, trackAround, wavyAt } from "./spinner-shape";

const GAP_AND_ENDS = (8 / TRACK_LENGTH) * 100;

/** The points a path goes through, its controls included. */
const numbers = (path: string) => path.match(/-?\d+(\.\d+)?/g)!.map(Number);

describe("the ring's shape", () => {
  it("is thirteen waves of three curves, begun at the crest on the right, flat or not alike", () => {
    expect(WAVY_PATH.match(/C/g)).toHaveLength(39);
    expect(numbers(FLAT_PATH)).toHaveLength(numbers(WAVY_PATH).length);
    const [x, y] = numbers(WAVY_PATH);
    expect(x).toBeCloseTo(32 + 0.4917 * 60, 1);
    expect(y).toBeCloseTo(32, 3);
  });

  it("closes on the point it began at", () => {
    const all = numbers(WAVY_PATH);
    expect(all.at(-2)).toBeCloseTo(all[0], 2);
    expect(all.at(-1)).toBeCloseTo(all[1], 2);
  });

  it("stays within the square", () => {
    for (const value of numbers(WAVY_PATH)) {
      expect(value).toBeGreaterThanOrEqual(0);
      expect(value).toBeLessThanOrEqual(64);
    }
  });
});

describe("wavyAt", () => {
  it("is flat up to a tenth and from ninety-five on", () => {
    expect(wavyAt(0)).toBe(false);
    expect(wavyAt(10)).toBe(false);
    expect(wavyAt(10.5)).toBe(true);
    expect(wavyAt(94)).toBe(true);
    expect(wavyAt(95)).toBe(false);
  });
});

describe("trackAround", () => {
  it("is the whole ring while nothing is lit", () => {
    expect(trackAround(0, false)).toEqual({ length: 100, from: 0 });
  });

  it("leaves the gap and the round ends on both sides of what is lit", () => {
    const track = trackAround(40, true)!;
    expect(track.from).toBeCloseTo(40 + GAP_AND_ENDS, 6);
    expect(track.length).toBeCloseTo(100 - 40 - 2 * GAP_AND_ENDS, 6);
  });

  it("is gone once what is lit and the gaps fill the ring", () => {
    expect(trackAround(97, false)).toBeNull();
    expect(trackAround(100, false)).toBeNull();
  });
});
