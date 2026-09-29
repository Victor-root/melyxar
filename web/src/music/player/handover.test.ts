import { describe, expect, it } from "vitest";
import { fadeBetween, handOverIn, secondsLeft } from "./handover";

describe("how long is left of the song playing", () => {
  it("reads the element when it knows the length", () => {
    expect(secondsLeft(200, 150, 0, 200)).toBe(50);
    expect(secondsLeft(200, 250, 0, 200)).toBe(0);
  });

  it("counts from where a converted song was asked for, when the element cannot", () => {
    expect(secondsLeft(Infinity, 30, 100, 200)).toBe(70);
    expect(secondsLeft(NaN, 30, 0, null)).toBeNull();
  });
});

describe("when the next song takes over", () => {
  it("starts just before the end, or as early as the crossfade asks", () => {
    expect(handOverIn(10, 0)).toBeCloseTo(9.98);
    expect(handOverIn(10, 4)).toBeCloseTo(5.98);
    expect(handOverIn(2, 4)).toBe(0);
  });

  it("never fades more than a third of the shorter song", () => {
    expect(fadeBetween(6, 200, 180)).toBe(6);
    expect(fadeBetween(6, 9, 200)).toBe(3);
    expect(fadeBetween(0, 200, 200)).toBe(0);
    expect(fadeBetween(6, null, null)).toBe(6);
  });
});
