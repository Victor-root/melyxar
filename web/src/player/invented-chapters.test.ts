import { describe, expect, it } from "vitest";
import type { PlaybackPlan, PlaybackThumbnails } from "../api";
import { chaptersOf, inventedMoments } from "./invented-chapters";

const SHEETS: PlaybackThumbnails = {
  url: "/sheets",
  every_seconds: 10,
  width: 160,
  height: 90,
  columns: 10,
  rows: 10,
  counted: 720,
};

/** Only what the chapters are worked out from. */
function plan(parts: Partial<PlaybackPlan>): PlaybackPlan {
  return { chapters: [], thumbnails: null, duration_minutes: null, ...parts } as PlaybackPlan;
}

describe("moments laid along a film", () => {
  it("starts at the beginning and keeps to round distances", () => {
    expect(inventedMoments(2 * 3600)).toEqual([0, 600, 1200, 1800, 2400, 3000, 3600, 4200, 4800, 5400, 6000, 6600]);
  });

  it("widens the distance for a long film so the row stays a size to read", () => {
    const moments = inventedMoments(3 * 3600 + 20 * 60);
    expect(moments.length).toBeLessThanOrEqual(12);
    expect(moments[1] - moments[0]).toBe(1200);
  });

  it("makes no card for the last minute of the film", () => {
    expect(inventedMoments(1500)).toEqual([0, 300, 600, 900, 1200]);
    expect(inventedMoments(1210)).toEqual([0, 300, 600, 900]);
  });

  it("makes nothing of a film too short to make a row, or of no length", () => {
    expect(inventedMoments(400)).toEqual([]);
    expect(inventedMoments(0)).toEqual([]);
    expect(inventedMoments(Number.NaN)).toEqual([]);
  });
});

describe("the chapters of a film's page", () => {
  it("keeps the chapters the file names and invents nothing beside them", () => {
    const named = [{ at_second: 0, title: "Opening" }];
    expect(chaptersOf(plan({ chapters: named, thumbnails: SHEETS }))).toBe(named);
  });

  it("invents them from the pictures when the file names none", () => {
    const found = chaptersOf(plan({ thumbnails: SHEETS, duration_minutes: 120 }));
    expect(found.length).toBe(12);
    expect(found.every((one) => one.title === null)).toBe(true);
    expect(found[1].at_second).toBe(600);
  });

  it("invents none without pictures to put on them", () => {
    expect(chaptersOf(plan({ duration_minutes: 120 }))).toEqual([]);
  });

  it("keeps to the shorter of what the file says and what the pictures cover", () => {
    // The pictures cover two hours and the file says half an hour.
    const found = chaptersOf(plan({ thumbnails: SHEETS, duration_minutes: 30 }));
    expect(found.at(-1)?.at_second).toBeLessThan(1800);
  });
});
