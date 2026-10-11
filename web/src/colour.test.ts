import { describe, expect, it } from "vitest";
import { fromHsl, fromHsv, hslOf, hsvOf, mostColourfulOf, parseHex } from "./colour";

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

describe("hsvOf and fromHsv", () => {
  it("take a colour apart and put it back as it was", () => {
    for (const colour of ["#c81e1e", "#1c7ed6", "#2f9e44", "#808080", "#000000", "#ffffff", "#ff0000"]) {
      const { hue, saturation, value } = hsvOf(colour);
      expect(fromHsv(hue, saturation, value)).toBe(colour);
    }
  });

  it("put the pure colours in a corner of the square", () => {
    expect(hsvOf("#00ff00")).toEqual({ hue: 120, saturation: 1, value: 1 });
    expect(hsvOf("#ffffff")).toEqual({ hue: 0, saturation: 0, value: 1 });
    expect(hsvOf("#000000").value).toBe(0);
  });
});

describe("parseHex", () => {
  it("reads six digits with or without the hash, in any case", () => {
    expect(parseHex("#C81E1E")).toBe("#c81e1e");
    expect(parseHex(" 1c7ed6 ")).toBe("#1c7ed6");
  });

  it("widens the short form", () => {
    expect(parseHex("#f80")).toBe("#ff8800");
  });

  it("refuses anything else", () => {
    for (const typed of ["", "#", "#12", "#12345", "#1234567", "#ggg000", "red"]) {
      expect(parseHex(typed)).toBeNull();
    }
  });
});
