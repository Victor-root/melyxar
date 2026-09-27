/*
 * The number a viewer watches while a film is on its way.
 */

import { describe, expect, it } from "vitest";
import { isLater, percentAt } from "./loading";

describe("the stages on the way to a playing film", () => {
  it("only ever move forward", () => {
    expect(isLater("producing", "session_opened")).toBe(true);
    expect(isLater("session_opened", "producing")).toBe(false);
    expect(isLater("producing", "producing")).toBe(false);
  });
});

describe("the number shown during a stage", () => {
  it("starts at the stage's own floor", () => {
    expect(percentAt("producing", 0)).toBe(24);
  });

  it("creeps toward the next stage and never reaches it", () => {
    const early = percentAt("producing", 1000);
    const late = percentAt("producing", 60_000);
    expect(early).toBeGreaterThan(24);
    expect(late).toBeGreaterThan(early);
    expect(late).toBeLessThan(58);
  });

  it("is the whole once the film is there", () => {
    expect(percentAt("done", 5000)).toBe(100);
  });
});
