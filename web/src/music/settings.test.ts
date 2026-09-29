import { describe, expect, it } from "vitest";
import { CEILINGS, ceilingOf, ceilingValue } from "./settings";

describe("the ceiling of a song's weight", () => {
  it("goes to the picker and back unchanged, none included", () => {
    for (const kbps of [null, ...CEILINGS]) {
      expect(ceilingOf(ceilingValue(kbps))).toBe(kbps);
    }
  });

  it("offers only what the server keeps as it is", () => {
    expect(Math.max(...CEILINGS)).toBe(320);
    expect(Math.min(...CEILINGS)).toBe(64);
  });
});
