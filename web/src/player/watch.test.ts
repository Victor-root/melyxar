import { describe, expect, it } from "vitest";
import { pictureBehindMs } from "./watch";

describe("pictureBehindMs", () => {
  it("is nothing when the picture shown is the one the clock is at", () => {
    expect(pictureBehindMs(10, 10.016, 1000, 1016, 1)).toBeCloseTo(0);
  });

  it("is positive when the picture runs behind the sound", () => {
    expect(pictureBehindMs(10, 9.5, 1000, 1000, 1)).toBeCloseTo(500);
  });

  it("carries the clock forward at the speed the film plays", () => {
    expect(pictureBehindMs(10, 10, 1000, 1100, 2)).toBeCloseTo(200);
  });
});
