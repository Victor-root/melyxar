import { describe, expect, it } from "vitest";
import { reliefFor, vividOf } from "./mark";

describe("vividOf", () => {
  it("gives the usual accent the red the logo was drawn in", () => {
    expect(vividOf("#c81e1e")).toBe("#e50000");
  });

  it("keeps the hue of any accent and makes it as vivid", () => {
    expect(vividOf("#1c7ed6")).toBe("#0079e6");
    expect(vividOf("#2f9e44")).toBe("#1fc73f");
  });

  it("leaves a grey grey", () => {
    expect(vividOf("#808080")).toBe("#737373");
  });
});

describe("reliefFor", () => {
  it("keeps the small relief while it has a pixel for each of the screen's", () => {
    expect(reliefFor(28, 2)).toBe("/melyxar-shade-64.png");
    expect(reliefFor(64, 1)).toBe("/melyxar-shade-64.png");
  });

  it("takes the large one as soon as the screen has more", () => {
    expect(reliefFor(28, 3)).toBe("/melyxar-shade-512.png");
    expect(reliefFor(64, 2.625)).toBe("/melyxar-shade-512.png");
  });

  it("takes the largest there is when even it falls short", () => {
    expect(reliefFor(320, 3)).toBe("/melyxar-shade-512.png");
  });
});
