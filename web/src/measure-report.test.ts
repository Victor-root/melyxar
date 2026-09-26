import { describe, expect, it } from "vitest";
import {
  frameStats,
  framesLost,
  heldFor,
  heldUpByTheMainThread,
  passesOf,
  percentile,
} from "./measure-report";

const at = (points: [number, number][]) => points.map(([time, top]) => ({ at: time, top }));

describe("passesOf", () => {
  it("cuts a scroll down, back up and down again into three passes", () => {
    const passes = passesOf(
      at([
        [0, 0],
        [100, 400],
        [200, 3000],
        [300, 2980],
        [400, 1500],
        [500, 0],
        [600, 1200],
        [700, 3000],
      ]),
    );
    expect(passes.map((pass) => [pass.direction, pass.startTop, pass.endTop])).toEqual([
      ["down", 0, 3000],
      ["up", 3000, 0],
      ["down", 0, 3000],
    ]);
  });

  it("ends a pass at a long stillness, and ignores a hand settling", () => {
    const passes = passesOf(
      at([
        [0, 0],
        [100, 900],
        [5000, 930],
        [5100, 1900],
      ]),
    );
    expect(passes.map((pass) => [pass.startTop, pass.endTop])).toEqual([
      [0, 900],
      [930, 1900],
    ]);
  });
});

describe("frames", () => {
  it("counts the frames a gap let go by on a screen of a given rate", () => {
    expect(framesLost(16.7, 16.7)).toBe(0);
    expect(framesLost(50, 16.7)).toBe(2);
    expect(framesLost(21, 6.94)).toBe(2);
  });

  it("sums a list of gaps", () => {
    const stats = frameStats([16, 17, 16, 50, 17], 16.7);
    expect(stats).toMatchObject({ frames: 5, lost: 2, stalls: 1, worst: 50 });
    expect(percentile([1, 2, 3, 4], 0.5)).toBe(2);
  });
});

describe("the main thread's share of a frame", () => {
  const drawn = (before: number, painting?: number) => ({ at: 0, gap: 16.7, before, painting });

  it("adds what came before the recorder's turn to what came after", () => {
    expect(heldFor(drawn(2, 5))).toBe(7);
    expect(heldFor(drawn(2))).toBe(2);
  });

  it("blames a late frame on the main thread only when the one before left no room", () => {
    expect(heldUpByTheMainThread(drawn(3, 12), 16.7)).toBe(true);
    expect(heldUpByTheMainThread(drawn(1, 4), 16.7)).toBe(false);
    expect(heldUpByTheMainThread(undefined, 16.7)).toBe(false);
  });
});
