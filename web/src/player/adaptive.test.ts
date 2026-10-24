import { describe, expect, it } from "vitest";
import {
  ADAPTIVE_RUNGS,
  UNMEASURED_RUNG,
  afterMoving,
  believed,
  letsThrough,
  nextMove,
  rungFor,
  startingOn,
  withMeasure,
} from "./adaptive";

const rungAt = (bitrate: number | null) => ADAPTIVE_RUNGS.findIndex((rung) => rung.bitrate === bitrate);

describe("the rung a connection carries", () => {
  it("takes the best rung that fits in part of the speed", () => {
    // Five megabits, the slow 4G of a browser's tools: 3.5 left, so 2.
    expect(ADAPTIVE_RUNGS[rungFor(5_000_000)].bitrate).toBe(2_000_000);
    expect(ADAPTIVE_RUNGS[rungFor(10_000_000)].bitrate).toBe(6_000_000);
    expect(ADAPTIVE_RUNGS[rungFor(30_000_000)].bitrate).toBe(20_000_000);
    expect(ADAPTIVE_RUNGS[rungFor(60_000_000)].bitrate).toBe(40_000_000);
  });

  it("offers the film as it is only to a fast connection", () => {
    expect(rungFor(200_000_000)).toBe(ADAPTIVE_RUNGS.length - 1);
    expect(ADAPTIVE_RUNGS[ADAPTIVE_RUNGS.length - 1].bitrate).toBeNull();
  });

  it("falls back on the lightest rung when nothing fits", () => {
    expect(rungFor(100_000)).toBe(0);
    expect(ADAPTIVE_RUNGS[0].height).toBe(360);
  });

  it("starts an unmeasured connection on a rung that plays nearly everywhere", () => {
    expect(ADAPTIVE_RUNGS[UNMEASURED_RUNG].bitrate).toBe(4_000_000);
  });
});

describe("the speed followed as segments arrive", () => {
  it("believes a drop at once and a rise only once it lasts", () => {
    let speed = withMeasure(null, 10_000_000, 0);
    speed = withMeasure(speed, 2_000_000, 2);
    expect(believed(speed)).toBeLessThan(7_000_000);

    let rising = withMeasure(null, 2_000_000, 0);
    rising = withMeasure(rising, 20_000_000, 0.5);
    expect(believed(rising)).toBeLessThan(5_000_000);
    for (let index = 0; index < 20; index += 1) {
      rising = withMeasure(rising, 20_000_000, 1);
    }
    expect(believed(rising)).toBeGreaterThan(18_000_000);
  });

  it("believes one segment that crawled in over a fast connection that fell", () => {
    // Measured, a connection cut from two hundred megabits to three took ten
    // segments to be believed when each weighed the same.
    let speed = withMeasure(null, 200_000_000, 0);
    speed = withMeasure(speed, 3_000_000, 8);
    expect(believed(speed)).toBeLessThan(20_000_000);
  });

  it("weighs a segment that arrived in no time very little", () => {
    let speed = withMeasure(null, 5_000_000, 0);
    speed = withMeasure(speed, 500_000_000, 0.01);
    expect(believed(speed)).toBeLessThan(6_000_000);
  });
});

describe("whether a rung lets the film through", () => {
  const film = { bitrate: 8_000_000, height: 720 };

  it("lets it through when the film is no heavier and no taller", () => {
    expect(letsThrough(film, rungAt(10_000_000))).toBe(true);
    expect(letsThrough(film, rungAt(6_000_000))).toBe(false);
    expect(letsThrough({ bitrate: 8_000_000, height: 2160 }, rungAt(10_000_000))).toBe(false);
    expect(letsThrough({ bitrate: 8_000_000, height: 2160 }, rungAt(40_000_000))).toBe(true);
  });

  it("always lets it through as it is, and never when nothing is known", () => {
    expect(letsThrough(null, ADAPTIVE_RUNGS.length - 1)).toBe(true);
    expect(letsThrough(null, rungAt(40_000_000))).toBe(false);
    expect(letsThrough({ bitrate: null, height: 720 }, rungAt(40_000_000))).toBe(false);
  });
});

describe("moving between rungs", () => {
  const sixMegabits = rungAt(6_000_000);
  // Heavier than every rung, so that every move changes the film.
  const heavy = { bitrate: 60_000_000, height: 2160 };
  const measured = (rung: number, at: number) => ({ ...startingOn(rung, at), measured: 5 });

  it("comes down at once when the connection falls short and little is held", () => {
    expect(nextMove(measured(sixMegabits, 0), 4_000_000, 5, 1_000, heavy)).toEqual({
      rung: rungAt(2_000_000),
      because: "slower_link",
    });
  });

  it("waits while plenty of film is held ahead", () => {
    expect(nextMove(measured(sixMegabits, 0), 4_000_000, 40, 1_000, heavy)).toBeNull();
  });

  it("believes no speed measured on too few segments", () => {
    expect(nextMove(startingOn(sixMegabits, 0), 1_000_000, 0, 1_000, heavy)).toBeNull();
  });

  it("comes down a rung after two stalls in a minute, whatever was measured", () => {
    const course = { ...startingOn(sixMegabits, 0), stalls: [10_000, 40_000] };
    expect(nextMove(course, null, 0, 45_000, heavy)).toEqual({ rung: sixMegabits - 1, because: "stalls" });
    const apart = { ...startingOn(sixMegabits, 0), stalls: [10_000, 100_000] };
    expect(nextMove(apart, null, 0, 105_000, heavy)).toBeNull();
  });

  it("goes nowhere below the lightest rung", () => {
    const course = { ...startingOn(0, 0), stalls: [1_000, 2_000] };
    expect(nextMove(course, 100_000, 0, 3_000, heavy)).toBeNull();
  });

  it("goes up only once the rung was kept a while and enough is held", () => {
    const course = measured(rungAt(2_000_000), 0);
    expect(nextMove(course, 30_000_000, 20, 10_000, heavy)).toBeNull();
    expect(nextMove(course, 30_000_000, 5, 40_000, heavy)).toBeNull();
    expect(nextMove(course, 30_000_000, 20, 40_000, heavy)).toEqual({
      rung: rungAt(20_000_000),
      because: "faster_link",
    });
  });

  it("waits longer before going back up after coming down", () => {
    const lowered = afterMoving(measured(sixMegabits, 0), { rung: rungAt(2_000_000), because: "slower_link" }, 0);
    expect(lowered.loweredAt).toBe(0);
    const tried = { ...lowered, measured: 5 };
    expect(nextMove(tried, 30_000_000, 20, 60_000, heavy)).toBeNull();
    expect(nextMove(tried, 30_000_000, 20, 130_000, heavy)?.because).toBe("faster_link");
  });

  it("never opens the film again for a rung that gives the same film", () => {
    const light = { bitrate: 8_000_000, height: 720 };
    const asItIs = { ...startingOn(ADAPTIVE_RUNGS.length - 1, 0), measured: 5 };
    // Sixty megabits fits the rung of forty, which lets this film through as
    // the film as it is does.
    expect(nextMove(asItIs, 60_000_000, 5, 1_000, light)).toBeNull();
    expect(nextMove(asItIs, 10_000_000, 5, 1_000, light)).toEqual({
      rung: sixMegabits,
      because: "slower_link",
    });
    const forty = { ...startingOn(rungAt(40_000_000), 0), measured: 5 };
    expect(nextMove(forty, 500_000_000, 20, 200_000, light)).toBeNull();
  });

  it("steps past the rungs that let the film through after stalls", () => {
    const light = { bitrate: 8_000_000, height: 720 };
    const course = { ...startingOn(ADAPTIVE_RUNGS.length - 1, 0), stalls: [1_000, 2_000] };
    expect(nextMove(course, null, 0, 3_000, light)).toEqual({ rung: sixMegabits, because: "stalls" });
  });

  it("starts each rung afresh", () => {
    const moved = afterMoving(
      { ...measured(sixMegabits, 0), stalls: [1_000] },
      { rung: sixMegabits + 1, because: "faster_link" },
      50_000,
    );
    expect(moved).toEqual({ rung: sixMegabits + 1, movedAt: 50_000, loweredAt: null, measured: 0, stalls: [] });
  });
});
