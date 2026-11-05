import { describe, expect, it } from "vitest";
import { steppedVolume } from "./steps";

describe("a step of volume", () => {
  it("goes a twentieth up or down, and lands on a step", () => {
    expect(steppedVolume(0.5, 1)).toBe(0.55);
    expect(steppedVolume(0.52, -1)).toBe(0.45);
  });

  it("never goes below nothing nor above everything", () => {
    expect(steppedVolume(0.02, -1)).toBe(0);
    expect(steppedVolume(0.98, 2)).toBe(1);
  });
});
