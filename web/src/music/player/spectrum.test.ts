import { describe, expect, it } from "vitest";
import { approach } from "./spectrum";

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
});
