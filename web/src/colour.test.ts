import { describe, expect, it } from "vitest";
import { fromHsl, hslOf, mostColourfulOf } from "./colour";

describe("hslOf and fromHsl", () => {
  it("take a colour apart and put it back as it was", () => {
    for (const colour of ["#c81e1e", "#1c7ed6", "#2f9e44", "#808080", "#000000", "#ffffff"]) {
      const { hue, saturation, lightness } = hslOf(colour);
      expect(fromHsl(hue, saturation, lightness)).toBe(colour);
    }
  });

  it("read the hue of the three primaries", () => {
    expect(hslOf("#ff0000").hue).toBe(0);
    expect(hslOf("#00ff00").hue).toBe(120);
    expect(hslOf("#0000ff").hue).toBe(240);
  });
});

describe("mostColourfulOf", () => {
  it("picks the brightest colour over a dark one of the same saturation", () => {
    expect(mostColourfulOf(["#200000", "#46241c", "#c81e1e"])).toBe("#c81e1e");
  });

  it("prefers a dull colour to a grey, and says nothing among none", () => {
    expect(mostColourfulOf(["#808080", "#43332f"])).toBe("#43332f");
    expect(mostColourfulOf([])).toBeNull();
  });
});
