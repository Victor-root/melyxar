import { describe, expect, it } from "vitest";
import { letterOfTheTopRow, type Placed } from "./letters";

/** Rows of cards 300 high with 50 between them, from the top of the page. */
function rows(...letters: (string | undefined)[][]): Placed[] {
  return letters.flatMap((row, index) =>
    row.map((letter) => ({ top: index * 350, height: 300, letter })),
  );
}

const read = (cards: Placed[], line: number) =>
  letterOfTheTopRow(cards.length, (index) => cards[index], line);

describe("letterOfTheTopRow", () => {
  const grid = rows(["a", "a", "a", "b"], ["b", "b", "c", "c"], ["c", "d", "d", "d"]);

  it("reads the row at the top of the screen", () => {
    expect(read(grid, 0)).toBe("a");
    expect(read(grid, 350)).toBe("c");
    expect(read(grid, 700)).toBe("d");
  });

  it("goes by what most of the row is filed under", () => {
    expect(read(rows(["a", "b", "b", "b"]), 0)).toBe("b");
  });

  it("takes the later letter when two share the row evenly", () => {
    expect(read(grid, 350)).toBe("c");
  });

  it("still counts a row scrolled up by less than a quarter of its height", () => {
    expect(read(grid, 350 + 60)).toBe("c");
    expect(read(grid, 350 + 90)).toBe("d");
  });

  it("says nothing past the last row", () => {
    expect(read(grid, 2000)).toBeNull();
    expect(read([], 0)).toBeNull();
  });

  it("leaves out a card nobody has filed yet", () => {
    expect(read(rows(["a", undefined, "b", "b"]), 0)).toBe("b");
  });
});
