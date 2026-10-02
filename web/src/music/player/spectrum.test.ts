import { describe, expect, it } from "vitest";
import { approach, levelAt } from "./spectrum";

describe("the levels of the spectrum", () => {
  it("glide towards their targets and say when they have got there", () => {
    const levels = [0, 0];
    expect(approach(levels, [1, 0.5])).toBe(false);
    expect(levels[0]).toBeGreaterThan(0);
    expect(levels[0]).toBeLessThan(1);
    for (let step = 0; step < 100; step++) {
      approach(levels, [1, 0.5]);
    }
    expect(approach(levels, [1, 0.5])).toBe(true);
  });

  it("are read between the two bands a place falls between", () => {
    expect(levelAt([0, 1, 0], 0)).toBe(0);
    expect(levelAt([0, 1, 0], 0.5)).toBe(1);
    expect(levelAt([0, 1, 0], 0.25)).toBe(0.5);
    expect(levelAt([0, 1, 0], 1)).toBe(0);
  });
});
