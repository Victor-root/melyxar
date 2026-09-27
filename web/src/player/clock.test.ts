import { describe, expect, it } from "vitest";
import { asClock, fromClock } from "./clock";

describe("a moment typed by hand", () => {
  it("reads minutes and seconds, hours too, and plain seconds", () => {
    expect(fromClock("1:02")).toBe(62);
    expect(fromClock(" 1:02:03 ")).toBe(3723);
    expect(fromClock("90")).toBe(90);
    expect(fromClock("0:07.5")).toBe(7.5);
  });

  it("refuses what is not a moment", () => {
    expect(fromClock("")).toBeNull();
    expect(fromClock("1:90")).toBeNull();
    expect(fromClock("a:02")).toBeNull();
    expect(fromClock("1:2:3:4")).toBeNull();
    expect(fromClock("-5")).toBeNull();
  });

  it("reads back what it writes", () => {
    for (const seconds of [0, 62, 599, 3723]) {
      expect(fromClock(asClock(seconds))).toBe(seconds);
    }
  });
});
