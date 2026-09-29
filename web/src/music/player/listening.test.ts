import { describe, expect, it } from "vitest";
import { countsAsListened } from "./listening";

describe("when a song counts as listened to", () => {
  it("counts once half of it has played", () => {
    expect(countsAsListened(99, 200)).toBe(false);
    expect(countsAsListened(100, 200)).toBe(true);
  });

  it("counts a long one after four minutes", () => {
    expect(countsAsListened(239, 1200)).toBe(false);
    expect(countsAsListened(240, 1200)).toBe(true);
  });

  it("never counts a song too short to say anything", () => {
    expect(countsAsListened(29, 29)).toBe(false);
  });
});
