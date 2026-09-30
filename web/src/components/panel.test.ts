import { describe, expect, it } from "vitest";
import { placeList } from "./panel";

const field = (top: number) =>
  ({ top, bottom: top + 40, left: 10, width: 200 }) as DOMRect;

describe("placeList", () => {
  it("opens under the field when there is room", () => {
    expect(placeList(field(100), 900)).toEqual({
      top: 146,
      left: 10,
      width: 200,
      room: 340,
    });
  });

  it("opens over the field near the bottom of the window", () => {
    expect(placeList(field(700), 800)).toEqual({
      bottom: 106,
      left: 10,
      width: 200,
      room: 340,
    });
  });

  it("never runs past the window on either side", () => {
    expect(placeList(field(150), 400).room).toBe(198);
  });

  it("grows a list away from the nearer edge of the window", () => {
    const right = {
      top: 100,
      bottom: 140,
      left: 1000,
      right: 1200,
      width: 200,
    } as DOMRect;
    expect(placeList(right, 900, 1300)).toEqual({
      top: 146,
      right: 100,
      width: 200,
      room: 340,
    });
    expect(placeList(field(100), 900, 1300).left).toBe(10);
  });
});
