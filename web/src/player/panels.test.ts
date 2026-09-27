/*
 * How far the words are shifted against the picture, as it is said and as a
 * nudge moves it.
 */

import { describe, expect, it } from "vitest";
import { nudged, offsetSaid } from "./panels";

describe("a shift of the words as a viewer reads it", () => {
  it("carries its sign and a tenth of a second", () => {
    expect(offsetSaid(1.5)).toBe("+1.5 s");
    expect(offsetSaid(-2)).toBe("-2.0 s");
  });

  it("is plain nought when there is none", () => {
    expect(offsetSaid(0)).toBe("0.0 s");
  });
});

describe("a nudge of the words", () => {
  it("moves them by the step asked", () => {
    expect(nudged(1, 0.5)).toBe(1.5);
    expect(nudged(1, -0.5)).toBe(0.5);
  });

  it("never goes past either end", () => {
    expect(nudged(14.8, 0.5)).toBe(15);
    expect(nudged(-15, -0.5)).toBe(-15);
  });
});
