import { describe, expect, it } from "vitest";
import { heardAt, speedAfter } from "./scrubbing";

describe("speedAfter", () => {
  it("chases the hand, faster the further ahead it is", () => {
    expect(speedAfter(0, 0.08, 1)).toBeCloseTo(1, 9);
    expect(speedAfter(0, 0.04, 1)).toBeCloseTo(0.5, 9);
  });

  it("goes backwards when the hand is behind", () => {
    expect(speedAfter(0, -0.08, 1)).toBeCloseTo(-1, 9);
  });

  it("changes speed smoothly rather than at once", () => {
    expect(speedAfter(0, 0.08, 0.01)).toBeCloseTo(1 / 3, 9);
    expect(speedAfter(1, 0, 0.015)).toBeCloseTo(0.5, 9);
  });

  it("never goes faster than four times either way", () => {
    expect(speedAfter(0, 10, 1)).toBe(4);
    expect(speedAfter(0, -10, 1)).toBe(-4);
  });
});

describe("heardAt", () => {
  it("is silent for a hand at rest, heard as soon as it moves", () => {
    expect(heardAt(0.01)).toBe(false);
    expect(heardAt(-0.2)).toBe(true);
  });
});
