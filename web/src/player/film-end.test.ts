import { describe, expect, it } from "vitest";
import { standsAtItsEnd, waitsAtItsLastPicture } from "./film-end";

describe("standsAtItsEnd", () => {
  const watch = { at: 1464.84, since: 0 };

  it("ends a film stood still for long enough just short of its length", () => {
    expect(standsAtItsEnd(watch, 1464.84, 2000, 1465.92, 0.07)).toBe(true);
    // Stood still with a third of a second still held, as seen on a real one.
    const short = { at: 1464.54, since: 0 };
    expect(standsAtItsEnd(short, 1464.54, 2000, 1465.92, 0.37)).toBe(true);
  });

  it("waits until it has stood still long enough", () => {
    expect(standsAtItsEnd(watch, 1464.84, 1999, 1465.92, 0.07)).toBe(false);
    expect(standsAtItsEnd(null, 1464.84, 5000, 1465.92, 0.07)).toBe(false);
    expect(standsAtItsEnd(watch, 1464.9, 5000, 1465.92, 0.07)).toBe(false);
  });

  it("leaves alone a film far from its end, or with film still held ahead", () => {
    expect(standsAtItsEnd({ at: 1400, since: 0 }, 1400, 5000, 1465.92, 0)).toBe(false);
    expect(standsAtItsEnd(watch, 1464.84, 5000, 1465.92, 1.2)).toBe(false);
    expect(standsAtItsEnd(watch, 1464.84, 5000, Number.NaN, 0)).toBe(false);
  });
});

describe("waitsAtItsLastPicture", () => {
  it("ends a film waiting at the last picture it holds", () => {
    expect(waitsAtItsLastPicture(1464.54, 1464.907)).toBe(true);
    expect(waitsAtItsLastPicture(1464.907, 1464.907)).toBe(true);
  });

  it("leaves a film waiting well before its last picture, or before it is held", () => {
    expect(waitsAtItsLastPicture(1460, 1464.907)).toBe(false);
    expect(waitsAtItsLastPicture(1464.54, null)).toBe(false);
  });
});
