import { describe, expect, it } from "vitest";
import { layingOrder, placed, rowsFor } from "./masonry";

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

  it("lays the tallest first, left to right, then each under the column ending highest", () => {
    expect(placed([card(10), card(50), card(20)], 2, 0)).toEqual([
      { column: 1, row: 20 },
      { column: 0, row: 0 },
      { column: 1, row: 0 },
    ]);
  });

  it("takes the leftmost of the columns ending about level", () => {
    // The third column ends twenty rows higher, which counts as level: the
    // card goes under the first rather than to the right.
    expect(placed([card(80), card(70), card(60), card(10)], 3, 25)[3]).toEqual({
      column: 0,
      row: 80,
    });
    expect(
      placed([card(80), card(70), card(20), card(10)], 3, 5)[3],
      "a real hole is still filled",
    ).toEqual({ column: 2, row: 20 });
  });

  it("keeps a wide card where it stands, starting below everything before it", () => {
    expect(placed([card(30), card(50), card(10, true), card(5)], 2, 0)).toEqual([
      { column: 1, row: 0 },
      { column: 0, row: 0 },
      { column: 0, row: 50 },
      { column: 0, row: 60 },
    ]);
  });

  it("stacks everything in one column, in the order given", () => {
    expect(placed([card(10), card(20)], 1, 0)).toEqual([
      { column: 0, row: 0 },
      { column: 0, row: 10 },
    ]);
  });
});

describe("a card that grows once the page is used", () => {
  const card = (rows: number) => ({ rows, wide: false });
  it("keeps its column when laid in the order it was laid before", () => {
    const before = [card(10), card(50), card(20)];
    const order = layingOrder(before, 2);
    const grown = [card(90), card(50), card(20)];
    expect(placed(grown, 2, 0, order)[0].column).toBe(placed(before, 2, 0)[0].column);
    expect(placed(grown, 2, 0)[0].column).not.toBe(placed(before, 2, 0)[0].column);
  });
});
