import { describe, expect, it } from "vitest";
import { rowsFor } from "./masonry";

describe("rowsFor", () => {
  it("covers the card and the gap under it, rounded up to whole rows", () => {
    expect(rowsFor(100, 24, 4)).toBe(31);
    expect(rowsFor(101, 24, 4)).toBe(32);
  });

  it("spans at least one row, even for nothing", () => {
    expect(rowsFor(0, 0, 4)).toBe(1);
  });
});
