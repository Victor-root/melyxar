import { describe, expect, it } from "vitest";
import { areaPieces, ceilingOf, linePieces, pointUnder } from "./curves";

describe("ceilingOf", () => {
  it("keeps a fixed ceiling, and otherwise leaves a little room above the top", () => {
    expect(ceilingOf([0.2, 0.9], 1)).toBe(1);
    expect(ceilingOf([10, 20, null])).toBeCloseTo(23);
    expect(ceilingOf([null, 0])).toBe(1);
  });
});

describe("linePieces", () => {
  it("lays the points across the width, nought at the foot", () => {
    expect(linePieces([0, 0.5, 1], 1, 100, 40)).toEqual(["M0 40 L50 20 L100 0"]);
  });

  it("breaks the line where nothing was measured", () => {
    expect(linePieces([1, null, 1, 1], 1, 30, 10)).toEqual(["M0 0", "M20 0 L30 0"]);
  });

  it("never draws above the ceiling", () => {
    expect(linePieces([2, 0], 1, 10, 10)).toEqual(["M0 0 L10 10"]);
  });
});

describe("areaPieces", () => {
  it("closes each run down to the foot", () => {
    expect(areaPieces([0.5, 0.5], 1, 10, 10)).toEqual(["M0 5 L10 5 L10 10 L0 10 Z"]);
  });
});

describe("pointUnder", () => {
  it("finds the nearest point to the pointer, and stays inside", () => {
    expect(pointUnder(0.5, 5)).toBe(2);
    expect(pointUnder(1.2, 5)).toBe(4);
    expect(pointUnder(-1, 5)).toBe(0);
    expect(pointUnder(0.5, 1)).toBe(0);
  });
});
