import { describe, expect, it } from "vitest";
import { menuPlace } from "./menu-place";

const size = { width: 300, height: 400 };
const window = { width: 1280, height: 800 };
const button = { top: 300, right: 290, bottom: 330, left: 260 };
const tile = { top: 100, right: 300, bottom: 340, left: 100 };

describe("menuPlace", () => {
  it("stands right of the tile when the window has the room", () => {
    expect(menuPlace(size, button, tile, window)).toEqual({ top: 100, left: 308 });
  });

  it("stands left of a tile at the right of the window", () => {
    const last = { ...tile, left: 900, right: 1100 };
    expect(menuPlace(size, button, last, window)).toEqual({ top: 100, left: 592 });
  });

  it("stays in the window at the foot of the screen", () => {
    const low = { ...tile, top: 600, bottom: 840 };
    expect(menuPlace(size, button, low, window).top).toBe(392);
  });

  it("goes back under the button when neither side has the room", () => {
    const narrow = { width: 500, height: 800 };
    expect(menuPlace(size, button, tile, narrow)).toEqual({ top: 336, left: 8 });
  });

  it("stands under the button with nothing to stand beside", () => {
    expect(menuPlace(size, button, undefined, window)).toEqual({ top: 336, left: 8 });
  });

  it("stands above the button when there is no room under it", () => {
    const low = { top: 700, right: 400, bottom: 730, left: 370 };
    expect(menuPlace(size, low, undefined, window)).toEqual({ top: 294, left: 100 });
  });
});
