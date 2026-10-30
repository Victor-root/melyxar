import { describe, expect, it } from "vitest";
import { hourOfDayOf, nextAt, stateAt, standingOf } from "./maintenance";

describe("maintenance as a page holds it", () => {
  it("counts down from the seconds the server gave, not from its clock", () => {
    const standing = standingOf(
      { state: "announced", starts_in_seconds: 90, until: null, message: "New disks" },
      1_000_000,
    );
    expect(standing.startsAt).toBe(1_090_000);
    expect(stateAt(standing, 1_089_999)).toBe("announced");
    expect(stateAt(standing, 1_090_000)).toBe("on");
  });

  it("keeps no countdown for what is not announced", () => {
    const on = standingOf({ state: "on", starts_in_seconds: null, until: null, message: null }, 5);
    expect(on.startsAt).toBeNull();
    expect(stateAt(on, 10)).toBe("on");
    const off = standingOf({ state: "off", starts_in_seconds: null, until: null, message: null }, 5);
    expect(stateAt(off, 10)).toBe("off");
  });

  it("ends at the next time the hour typed comes, today or tomorrow", () => {
    const now = new Date(2026, 9, 30, 14, 0).getTime();
    expect(nextAt("16:30", now)).toEqual(new Date(2026, 9, 30, 16, 30));
    expect(nextAt("09:15", now)).toEqual(new Date(2026, 9, 31, 9, 15));
    expect(nextAt("14:00", now)).toEqual(new Date(2026, 9, 31, 14, 0));
    expect(hourOfDayOf(new Date(2026, 9, 30, 7, 5).toISOString())).toBe("07:05");
  });
});
