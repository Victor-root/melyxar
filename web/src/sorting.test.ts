/*
 * A list reordered by hand, as the arithmetic behind the drag sees it.
 */

import { describe, expect, it } from "vitest";
import { landingPlace, leftToSettle, movedWithin, stepAside } from "./sorting";

/** Five lines of fifty, one under the other. */
const LINES = [0, 50, 100, 150, 200].map((top) => ({ top, height: 50 }));

describe("where a dragged line lands", () => {
  it("stays put until it has crossed half of a neighbour", () => {
    expect(landingPlace(LINES, 1, 0)).toBe(1);
    expect(landingPlace(LINES, 1, 24)).toBe(1);
    expect(landingPlace(LINES, 1, 26)).toBe(2);
    expect(landingPlace(LINES, 1, -26)).toBe(0);
  });

  it("goes no further than either end, however far the hand goes", () => {
    expect(landingPlace(LINES, 0, 10_000)).toBe(4);
    expect(landingPlace(LINES, 4, -10_000)).toBe(0);
  });
});

describe("the lines stepping aside", () => {
  it("moves the ones passed over by the height of the line held, and no others", () => {
    // The second line dragged down to the fourth place.
    expect([0, 1, 2, 3, 4].map((index) => stepAside(index, 1, 3, 110, 50))).toEqual([
      0, 110, -50, -50, 0,
    ]);
    // The fourth dragged up to the first place.
    expect([0, 1, 2, 3, 4].map((index) => stepAside(index, 3, 0, -140, 50))).toEqual([
      50, 50, 50, -140, 0,
    ]);
  });
});

describe("a line dropped", () => {
  it("glides from where the hand left it to its new place", () => {
    // Dragged down by 110 to the fourth place, whose top is then 150.
    expect(leftToSettle(LINES, 1, 3, 110)).toBe(10);
    // Dragged up by 140 to the first place.
    expect(leftToSettle(LINES, 3, 0, -140)).toBe(10);
    expect(leftToSettle(LINES, 2, 2, 12)).toBe(12);
  });

  it("is taken out of its place and put at the other", () => {
    expect(movedWithin(["a", "b", "c", "d"], 0, 2)).toEqual(["b", "c", "a", "d"]);
    expect(movedWithin(["a", "b", "c", "d"], 3, 1)).toEqual(["a", "d", "b", "c"]);
    expect(movedWithin(["a", "b"], 1, 1)).toEqual(["a", "b"]);
  });
});
