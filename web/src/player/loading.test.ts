/*
 * The number a viewer watches while a film is on its way.
 */

import { describe, expect, it } from "vitest";
import { glideToward, isLater, percentOf } from "./loading";

describe("the stages on the way to a playing film", () => {
  it("only ever move forward", () => {
    expect(isLater("producing", "session_opened")).toBe(true);
    expect(isLater("session_opened", "producing")).toBe(false);
    expect(isLater("producing", "producing")).toBe(false);
  });
});

describe("the number worth what was measured", () => {
  it("is nothing before anything happened", () => {
    expect(percentOf({ stage: "opening", written: 0, carried: 0 })).toBe(0);
  });

  it("climbs with the first piece being written", () => {
    const quarter = percentOf({ stage: "producing", written: 0.25, carried: 0 });
    const half = percentOf({ stage: "producing", written: 0.5, carried: 0 });
    expect(quarter).toBeGreaterThan(8);
    expect(half).toBeGreaterThan(quarter);
    expect(percentOf({ stage: "producing", written: 1, carried: 0 })).toBe(85);
  });

  it("climbs on with that piece arriving", () => {
    const half = percentOf({ stage: "produced", written: 1, carried: 0.5 });
    expect(half).toBeGreaterThan(85);
    expect(half).toBeLessThan(98);
    expect(percentOf({ stage: "produced", written: 1, carried: 1 })).toBe(98);
  });

  it("is the whole once the film is there, and only then", () => {
    expect(percentOf({ stage: "first_fragment_loaded", written: 1, carried: 1 })).toBe(98);
    expect(percentOf({ stage: "done", written: 1, carried: 1 })).toBe(100);
  });
});

describe("the number shown", () => {
  it("glides toward what was measured without passing it", () => {
    const next = glideToward(10, 50);
    expect(next).toBeGreaterThan(10);
    expect(next).toBeLessThan(50);
    expect(glideToward(49.9, 50)).toBe(50);
  });

  it("never goes back", () => {
    expect(glideToward(60, 40)).toBe(60);
  });
});
