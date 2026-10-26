import { describe, expect, it } from "vitest";

import { APART, TOGETHER_GRACE_SECONDS, together } from "./doubled";

describe("together", () => {
  it("says nothing while only one deck sounds", () => {
    expect(together(APART, false, 1000, 0)).toEqual({ now: APART, seconds: 0, say: false });
  });

  it("says nothing within the fade and its grace", () => {
    const first = together(APART, true, 0, 3);
    const within = together(first.now, true, (3 + TOGETHER_GRACE_SECONDS) * 1000, 3);
    expect(within.say).toBe(false);
    expect(within.seconds).toBe(3 + TOGETHER_GRACE_SECONDS);
  });

  it("says it once when both sound past the fade", () => {
    const first = together(APART, true, 0, 0);
    const past = together(first.now, true, 2500, 0);
    expect(past.say).toBe(true);
    expect(together(past.now, true, 9000, 0).say).toBe(false);
  });

  it("says it again for another stretch", () => {
    const past = together(together(APART, true, 0, 0).now, true, 2500, 0);
    const apart = together(past.now, false, 3000, 0);
    const again = together(together(apart.now, true, 10_000, 0).now, true, 12_500, 0);
    expect(again.say).toBe(true);
    expect(again.seconds).toBe(2.5);
  });
});
