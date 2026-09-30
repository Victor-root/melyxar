import { describe, expect, it } from "vitest";
import { LONGEST_QUEUE, shuffleOffset } from "./queueing";

describe("where a shuffled queue starts", () => {
  it("takes the whole of a library that fits in a queue", () => {
    expect(shuffleOffset(0, 0.9)).toBe(0);
    expect(shuffleOffset(LONGEST_QUEUE, 0.99)).toBe(0);
  });

  it("starts anywhere a whole queue still fits in a larger library", () => {
    const total = LONGEST_QUEUE + 500;
    expect(shuffleOffset(total, 0)).toBe(0);
    expect(shuffleOffset(total, 0.9999)).toBe(500);
    expect(shuffleOffset(total, 0.5)).toBe(250);
  });
});
