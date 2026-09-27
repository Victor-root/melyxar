import { describe, expect, it } from "vitest";
import { placed, rowsFor } from "./masonry";

describe("rowsFor", () => {
  it("covers the card and the gap under it, rounded up to whole rows", () => {
    expect(rowsFor(100, 24, 4)).toBe(31);
    expect(rowsFor(101, 24, 4)).toBe(32);
  });

  it("spans at least one row, even for nothing", () => {
    expect(rowsFor(0, 0, 4)).toBe(1);
  });
});

describe("placed", () => {
  const card = (rows: number, wide = false) => ({ rows, wide });

  it("puts a card under the column ending highest", () => {
    expect(placed([card(50), card(10), card(20)], 2, 0)).toEqual([
      { column: 0, row: 0 },
      { column: 1, row: 0 },
      { column: 1, row: 10 },
    ]);
  });

  it("takes the leftmost of the columns ending about level", () => {
    // The middle one ends ten rows higher, which counts as level: the card
    // goes under the first column rather than into the middle.
    expect(placed([card(60), card(50), card(80), card(30)], 3, 25)[3]).toEqual({
      column: 0,
      row: 60,
    });
    expect(
      placed([card(60), card(20), card(80), card(30)], 3, 5)[3],
      "a real hole is still filled",
    ).toEqual({ column: 1, row: 20 });
  });

  it("starts a wide card below everything, and what follows below it", () => {
    expect(placed([card(30), card(50), card(10, true), card(5)], 2, 0)).toEqual([
      { column: 0, row: 0 },
      { column: 1, row: 0 },
      { column: 0, row: 50 },
      { column: 0, row: 60 },
    ]);
  });

  it("stacks everything in one column", () => {
    expect(placed([card(10), card(20)], 1, 0)).toEqual([
      { column: 0, row: 0 },
      { column: 0, row: 10 },
    ]);
  });
});
