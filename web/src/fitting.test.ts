import { describe, expect, it } from "vitest";
import { AS_IT_IS, FEWEST_LINES, nextFit } from "./fitting";

describe("nextFit", () => {
  it("leaves a text that fits as it is", () => {
    expect(nextFit(AS_IT_IS, 200, 200.5, 26)).toBe(AS_IT_IS);
  });

  it("lets the column out before cutting anything", () => {
    expect(nextFit(AS_IT_IS, 300, 200, 26)).toEqual({ wide: true, lines: null });
  });

  it("keeps a text that fits once let out", () => {
    const wide = { wide: true, lines: null };
    expect(nextFit(wide, 190, 200, 26)).toBe(wide);
  });

  it("cuts to whole lines, one given to the word that opens the rest", () => {
    // Seven lines of room, six of text and the word under them.
    expect(nextFit({ wide: true, lines: null }, 400, 190, 26)).toEqual({ wide: true, lines: 6 });
  });

  it("never cuts to fewer lines than a synopsis needs", () => {
    expect(nextFit({ wide: true, lines: null }, 400, 20, 26).lines).toBe(FEWEST_LINES);
  });

  it("is settled once cut", () => {
    const cut = { wide: true, lines: 4 };
    expect(nextFit(cut, 400, 100, 26)).toBe(cut);
  });
});
