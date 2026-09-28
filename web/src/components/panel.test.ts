import { describe, expect, it } from "vitest";
import { placeList } from "./panel";

const field = (top: number) => ({ top, bottom: top + 40, left: 10, width: 200 }) as DOMRect;

describe("placeList", () => {
  it("opens under the field when there is room", () => {
    expect(placeList(field(100), 900)).toEqual({ top: 146, left: 10, width: 200, room: 340 });
  });

  it("opens over the field near the bottom of the window", () => {
    expect(placeList(field(700), 800)).toEqual({ bottom: 106, left: 10, width: 200, room: 340 });
  });

  it("never runs past the window on either side", () => {
    expect(placeList(field(150), 400).room).toBe(198);
  });
});
