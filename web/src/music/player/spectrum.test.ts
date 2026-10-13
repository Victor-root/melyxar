import { describe, expect, it } from "vitest";
import type { SongSpectrum } from "../api";
import { BarMeter, approach, barsOf, levelsAt, momentOf } from "./spectrum-data";

/** Two bands, four readings a second: loud then quiet then loud again. */
const SPECTRUM: SongSpectrum = {
  bands: 2,
  framesASecond: 4,
  levels: Uint8Array.from([255, 0, 0, 255, 255, 0]),
};

function at(seconds: number): number[] {
  const into = [0, 0];
  levelsAt(SPECTRUM, seconds, into);
  return into;
}

describe("the levels of the wave", () => {
  it("are those of a reading at the middle of the stretch it was made over", () => {
    expect(at(0.125)).toEqual([1, 0]);
    expect(at(0.375)).toEqual([0, 1]);
  });

  it("are read between two readings where a moment falls between them", () => {
    const [first, second] = at(0.25);
    expect(first).toBeCloseTo(0.5);
    expect(second).toBeCloseTo(0.5);
  });

  it("hold at the nearest reading before the first and after the last", () => {
    expect(at(0)).toEqual([1, 0]);
    expect(at(99)).toEqual([1, 0]);
  });
});

describe("the glide of the wave", () => {
  it("goes towards its targets and says when it has got there", () => {
    const levels = [0, 0];
    expect(approach(levels, [1, 0.5])).toBe(false);
    expect(levels[0]).toBeGreaterThan(0);
    expect(levels[0]).toBeLessThan(1);
    for (let step = 0; step < 100; step += 1) {
      approach(levels, [1, 0.5]);
    }
    expect(approach(levels, [1, 0.5])).toBe(true);
  });
});

describe("where the song is", () => {
  it("is in the first quarter of the second the player last said", () => {
    expect(momentOf(10, 0)).toBeCloseTo(10.125);
  });

  it("moves on with the time since, never past the next second", () => {
    expect(momentOf(10, 400)).toBeCloseTo(10.525);
    expect(momentOf(10, 5_000)).toBeCloseTo(11.125);
    expect(momentOf(10, -50)).toBeCloseTo(10.125);
  });
});

describe("the bars of the button of the song playing", () => {
  it("share the bands out in order, each the loudest of its share", () => {
    expect(barsOf([1, 0.2, 0.6, 0.4, 0.3, 0.2, 0, 0.05])).toEqual([1, 0.6, 0.3, 0.05]);
    expect(barsOf([])).toEqual([0, 0, 0, 0]);
  });

  it("move a song kept to a narrow range all the way up and down", () => {
    const meter = new BarMeter();
    let shown: readonly number[] = [];
    for (let step = 0; step < 60; step += 1) {
      const level = step % 2 === 0 ? 0.55 : 0.65;
      shown = meter.step([level, level, level, level], 33);
    }
    const up = [...meter.step([0.65, 0.65, 0.65, 0.65], 33)];
    const down = [...meter.step([0.55, 0.55, 0.55, 0.55], 400)];
    expect(shown.length).toBe(4);
    expect(up[0]).toBeGreaterThan(0.75);
    expect(down[0]).toBeLessThan(0.3);
  });

  it("jump up at once and fall back slowly", () => {
    const meter = new BarMeter();
    meter.step([0, 0, 0, 0], 33);
    const up = meter.step([1, 1, 1, 1], 33)[0];
    expect(up).toBeGreaterThan(0.6);
    const fallen = meter.step([0, 0, 0, 0], 100)[0];
    expect(fallen).toBeGreaterThan(up - 0.2);
    expect(fallen).toBeLessThan(up);
  });

  it("never stand quite flat", () => {
    const meter = new BarMeter();
    for (let step = 0; step < 30; step += 1) {
      meter.step([0, 0, 0, 0], 100);
    }
    expect(meter.step([0, 0, 0, 0], 100)).toEqual([0.15, 0.15, 0.15, 0.15]);
  });
});
