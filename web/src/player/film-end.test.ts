import { describe, expect, it } from "vitest";
import { standsAtItsEnd } from "./film-end";

describe("standsAtItsEnd", () => {
  const watch = { at: 1464.84, since: 0 };

  it("ends a film stood still for long enough just short of its length", () => {
    expect(standsAtItsEnd(watch, 1464.84, 2000, 1465.92, 0.07)).toBe(true);
  });

  it("waits until it has stood still long enough", () => {
    expect(standsAtItsEnd(watch, 1464.84, 1999, 1465.92, 0.07)).toBe(false);
    expect(standsAtItsEnd(null, 1464.84, 5000, 1465.92, 0.07)).toBe(false);
    expect(standsAtItsEnd(watch, 1464.9, 5000, 1465.92, 0.07)).toBe(false);
  });

  it("leaves alone a film far from its end, or with film still held ahead", () => {
    expect(standsAtItsEnd({ at: 1400, since: 0 }, 1400, 5000, 1465.92, 0)).toBe(false);
    expect(standsAtItsEnd(watch, 1464.84, 5000, 1465.92, 1)).toBe(false);
    expect(standsAtItsEnd(watch, 1464.84, 5000, Number.NaN, 0)).toBe(false);
  });
});
