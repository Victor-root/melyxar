import { describe, expect, it } from "vitest";
import { clocksApart } from "./scrubbing";

describe("clocksApart", () => {
  it("reads how far ahead the clock of the sound is from what it says it plays, and when", () => {
    expect(clocksApart({ contextTime: 12.5, performanceTime: 40_000 }, 40_050, 12.6, 0.05)).toBeCloseTo(-27.5, 9);
  });

  it("goes by where the sound is made, less the time it takes to be heard, when what it says is from before a pause", () => {
    expect(clocksApart({ contextTime: 12.5, performanceTime: 40_000 }, 100_000, 12.6, 0.05)).toBeCloseTo(-87.45, 9);
  });

  it("goes by where the sound is made while it has said nothing yet", () => {
    expect(clocksApart({ contextTime: 0, performanceTime: 0 }, 3_000, 0.2, 0.05)).toBeCloseTo(-2.85, 9);
  });
});
