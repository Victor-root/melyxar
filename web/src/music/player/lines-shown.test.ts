import { describe, expect, it } from "vitest";
import { MARGIN, STEP, keeping, linesAround, linesShown } from "./lines-shown";

describe("the lines of a long list that are made", () => {
  it("makes those in sight and a margin around them, on whole steps", () => {
    // Lines of 50 points, the box scrolled to line 100 and showing 16 lines.
    const shown = linesShown(5000, 800, 0, 50, 2000);
    expect(shown.from).toBeLessThanOrEqual(100 - MARGIN);
    expect(shown.to).toBeGreaterThanOrEqual(116 + MARGIN);
    expect(shown.from % STEP).toBe(0);
    expect(shown.to % STEP).toBe(0);
    expect(shown.to - shown.from).toBeLessThanOrEqual(16 + 2 * MARGIN + 2 * STEP);
  });

  it("counts from where the list begins in the box", () => {
    expect(linesShown(5000, 800, 1000, 50, 2000)).toEqual(linesShown(4000, 800, 0, 50, 2000));
  });

  it("does not change while the scroll stays within a step", () => {
    expect(linesShown(5000, 800, 0, 50, 2000)).toEqual({ from: 80, to: 140 });
    expect(linesShown(5150, 800, 0, 50, 2000)).toEqual({ from: 80, to: 140 });
    expect(linesShown(5500, 800, 0, 50, 2000)).toEqual({ from: 90, to: 150 });
  });

  it("stays within the list at both ends", () => {
    expect(linesShown(0, 800, 0, 50, 2000).from).toBe(0);
    expect(linesShown(-300, 800, 0, 50, 2000).from).toBe(0);
    expect(linesShown(99_000, 800, 0, 50, 2000).to).toBe(2000);
    expect(linesShown(0, 800, 0, 50, 7)).toEqual({ from: 0, to: 7 });
  });

  it("still makes the last lines when the box is scrolled past the list", () => {
    const shown = linesShown(500_000, 800, 0, 50, 2000);
    expect(shown.to).toBe(2000);
    expect(shown.from).toBeLessThan(2000);
  });

  it("makes a first few before the height of a line is known", () => {
    expect(linesShown(0, 800, 0, 0, 2000)).toEqual({ from: 0, to: STEP });
    expect(linesShown(0, 800, 0, 0, 3)).toEqual({ from: 0, to: 3 });
  });
});

describe("the lines made around one before anything is measured", () => {
  it("holds the line and fills a screen on either side of it", () => {
    const shown = linesAround(1000, 2000);
    expect(shown.from).toBeLessThanOrEqual(1000 - 2 * MARGIN);
    expect(shown.to).toBeGreaterThanOrEqual(1000 + 2 * MARGIN);
  });

  it("stays within the list", () => {
    expect(linesAround(0, 2000).from).toBe(0);
    expect(linesAround(1999, 2000).to).toBe(2000);
    expect(linesAround(0, 5)).toEqual({ from: 0, to: 5 });
  });
});

describe("a line kept among those made", () => {
  it("widens the lines made just enough to hold it", () => {
    expect(keeping({ from: 100, to: 160 }, 40)).toEqual({ from: 40, to: 160 });
    expect(keeping({ from: 100, to: 160 }, 200)).toEqual({ from: 100, to: 201 });
    expect(keeping({ from: 100, to: 160 }, 120)).toEqual({ from: 100, to: 160 });
    expect(keeping({ from: 100, to: 160 }, null)).toEqual({ from: 100, to: 160 });
  });
});
