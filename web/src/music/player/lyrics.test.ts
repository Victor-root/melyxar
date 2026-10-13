import { describe, expect, it } from "vitest";
import { FURTHEST_OFFSET_MS, lineAt, litLineAt, nudgedOffset, shiftedLines } from "./lyrics";

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

describe("the lines moved by hand", () => {
  it("move every line, none before the start", () => {
    expect(shiftedLines(lines, 1500).map((line) => line.at_ms)).toEqual([2500, 6500, 6500, 10_500]);
    expect(shiftedLines(lines, -2000).map((line) => line.at_ms)).toEqual([0, 3000, 3000, 7000]);
  });

  it("are the same lines when nothing moves", () => {
    expect(shiftedLines(lines, 0)).toBe(lines);
  });

  it("are moved by a press no further than the furthest", () => {
    expect(nudgedOffset(1000, 500)).toBe(1500);
    expect(nudgedOffset(FURTHEST_OFFSET_MS - 200, 500)).toBe(FURTHEST_OFFSET_MS);
    expect(nudgedOffset(-FURTHEST_OFFSET_MS, -500)).toBe(-FURTHEST_OFFSET_MS);
  });
});
