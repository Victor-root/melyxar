import { describe, expect, it } from "vitest";
import { columnsOfFaces } from "./door-columns";

describe("columnsOfFaces", () => {
  it("keeps a few faces on one line", () => {
    expect([1, 2, 3].map(columnsOfFaces)).toEqual([1, 2, 3]);
  });

  it("fills every line of an even number the width allows", () => {
    expect(columnsOfFaces(4)).toBe(2);
    expect(columnsOfFaces(6)).toBe(3);
    expect(columnsOfFaces(8)).toBe(4);
    expect(columnsOfFaces(10)).toBe(5);
    expect(columnsOfFaces(12)).toBe(4);
  });

  it("leaves the last line short for the rest", () => {
    expect(columnsOfFaces(5)).toBe(3);
    expect(columnsOfFaces(7)).toBe(4);
    expect(columnsOfFaces(14)).toBe(4);
  });
});
