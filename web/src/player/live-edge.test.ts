import { describe, expect, it } from "vitest";
import { BEHIND_AT_MOST, liveRate } from "./live-edge";

describe("liveRate", () => {
  it("plays as it comes when at now", () => {
    expect(liveRate(0)).toBe(1);
    expect(liveRate(BEHIND_AT_MOST)).toBe(1);
  });

  it("plays faster the further behind, in steps", () => {
    expect(liveRate(BEHIND_AT_MOST + 0.2)).toBeCloseTo(1.1);
    expect(liveRate(BEHIND_AT_MOST + 0.5)).toBeCloseTo(1.25);
  });

  it("never plays faster than half again", () => {
    expect(liveRate(BEHIND_AT_MOST + 2.5)).toBeCloseTo(1.5);
  });
});
