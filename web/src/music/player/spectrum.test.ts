import { describe, expect, it } from "vitest";
import type { SongSpectrum } from "../api";
import { approach, levelsAt, momentOf } from "./spectrum-data";

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
