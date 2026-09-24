import { beforeEach, describe, expect, it } from "vitest";
import { forgetKept, keep, recall } from "./kept";

describe("kept", () => {
  beforeEach(forgetKept);

  it("gives back what a screen was answered, nothing included", () => {
    keep("home", { rows: 3 });
    keep("work:none", null);
    expect(recall("home")).toEqual({ value: { rows: 3 } });
    expect(recall("work:none")).toEqual({ value: null });
    expect(recall("never")).toBeUndefined();
  });

  it("forgets the oldest past forty, and everything when asked", () => {
    for (let screen = 0; screen < 41; screen += 1) {
      keep(`screen:${screen}`, screen);
    }
    expect(recall("screen:0")).toBeUndefined();
    expect(recall("screen:40")).toEqual({ value: 40 });
    forgetKept();
    expect(recall("screen:40")).toBeUndefined();
  });
});
