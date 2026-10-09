import { describe, expect, it } from "vitest";
import { lineAt, litLineAt } from "./lyrics";

const lines = [1000, 5000, 5000, 9000].map((at_ms, index) => ({ at_ms, text: String(index) }));

describe("the line being sung", () => {
  it("is none before the first, then the last one whose moment has come", () => {
    expect(lineAt(lines, 0)).toBe(-1);
    expect(lineAt(lines, 1000)).toBe(0);
    expect(lineAt(lines, 4999)).toBe(0);
    expect(lineAt(lines, 5000)).toBe(2);
    expect(lineAt(lines, 60_000)).toBe(3);
    expect(lineAt([], 5000)).toBe(-1);
  });
});

describe("the line lit", () => {
  it("comes a quarter of a second before the line is sung", () => {
    expect(litLineAt(lines, 0.7)).toBe(-1);
    expect(litLineAt(lines, 0.75)).toBe(0);
    expect(litLineAt(lines, 4.75)).toBe(2);
  });
});
