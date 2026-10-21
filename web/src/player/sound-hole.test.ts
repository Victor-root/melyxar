import { describe, expect, it } from "vitest";
import { STILL_FOR, watchStill } from "./sound-hole";

describe("a film standing still", () => {
  it("is stuck once it has stood still long enough with film held ahead", () => {
    const first = watchStill(null, 305.975, 0, 44);
    expect(first.stuck).toBe(false);
    expect(watchStill(first.watch, 305.975, STILL_FOR - 1, 44).stuck).toBe(false);
    expect(watchStill(first.watch, 305.975, STILL_FOR, 44).stuck).toBe(true);
  });

  it("is only waiting for the network when nothing is held ahead", () => {
    const first = watchStill(null, 12, 0, 0.2);
    expect(watchStill(first.watch, 12, STILL_FOR * 3, 0.2).stuck).toBe(false);
  });

  it("starts counting again as soon as it moves", () => {
    const first = watchStill(null, 12, 0, 30);
    const moved = watchStill(first.watch, 12.5, STILL_FOR, 30);
    expect(moved.stuck).toBe(false);
    expect(moved.watch).toEqual({ at: 12.5, since: STILL_FOR });
  });
});
