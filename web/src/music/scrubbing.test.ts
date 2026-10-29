import { describe, expect, it } from "vitest";
import { grainOf } from "./scrubbing";

describe("grainOf", () => {
  it("plays what the hand went over in the time it took", () => {
    const even = grainOf(10, 10.05, 50, 180);
    expect(even?.offset).toBe(10);
    expect(even?.backwards).toBe(false);
    expect(even?.covers).toBeCloseTo(0.05, 9);
    expect(even?.rate).toBeCloseTo(1, 9);
    expect(even?.lasts).toBeCloseTo(0.05, 9);
    const slow = grainOf(10, 10.01, 40, 180);
    expect(slow?.rate).toBeCloseTo(0.25, 6);
    expect(slow?.lasts).toBeCloseTo(0.04, 6);
  });

  it("plays backwards, read from the end, when the hand goes back", () => {
    const back = grainOf(10, 9.95, 50, 180);
    expect(back?.backwards).toBe(true);
    expect(back?.offset).toBe(170);
    expect(back?.covers).toBeCloseTo(0.05, 6);
  });

  it("keeps within the fastest and the slowest, a long rest counting as a short move", () => {
    expect(grainOf(0, 2, 50, 180)?.rate).toBe(4);
    expect(grainOf(0, 0.003, 2_000, 180)?.rate).toBe(0.05);
    expect(grainOf(0, 0.06, 2_000, 180)?.rate).toBeCloseTo(0.5, 6);
  });

  it("plays nothing for a hand that did not move", () => {
    expect(grainOf(5, 5.001, 30, 180)).toBeNull();
  });
});
