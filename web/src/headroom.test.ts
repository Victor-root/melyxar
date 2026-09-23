import { describe, expect, it } from "vitest";
import { headroomAt } from "./headroom";
import type { Headroom } from "./headroom";

const top = 60;

/** Where the bar is after the page went through each of these positions. */
function after(...positions: number[]): Headroom {
  return positions.reduce((bar, y) => headroomAt(bar, y, top), {
    shown: true,
    turnedAt: 0,
  } as Headroom);
}

describe("headroomAt", () => {
  it("is always out at the top of the page", () => {
    expect(after(0, 40, 60).shown).toBe(true);
    expect(after(0, 500, 900, 30).shown).toBe(true);
  });

  it("steps aside once the page is read down, not at the first shake", () => {
    expect(after(60, 65).shown).toBe(true);
    expect(after(60, 80).shown).toBe(false);
  });

  it("comes back at the first move up", () => {
    expect(after(200, 600, 590).shown).toBe(true);
    expect(after(200, 600, 598).shown).toBe(false);
  });

  it("stays out after a move up until the page is read down again", () => {
    expect(after(200, 600, 580, 588).shown).toBe(true);
    expect(after(200, 600, 580, 600).shown).toBe(false);
  });

  it("measures the way back up from the lowest the page went", () => {
    expect(after(200, 600, 900, 890).shown).toBe(true);
  });
});
