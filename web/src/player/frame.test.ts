import { describe, expect, it } from "vitest";
import { framed, SMALLEST } from "./frame";
import type { Frame } from "./frame";

const ROOM = { width: 1000, height: 600 };
const FROM: Frame = { left: 600, top: 100, width: 300, height: 400 };

describe("a window on the picture", () => {
  it("moves by its title bar as far as the hand goes", () => {
    expect(framed("move", FROM, ROOM, -200, 50)).toEqual({ ...FROM, left: 400, top: 150 });
  });

  it("is moved no further than the picture's edges", () => {
    expect(framed("move", FROM, ROOM, 500, -500)).toEqual({ ...FROM, left: 700, top: 0 });
  });

  it("grows from its right edge and keeps its left where it was", () => {
    expect(framed("e", FROM, ROOM, 50, 0)).toEqual({ ...FROM, width: 350 });
  });

  it("grows from its left edge and keeps its right where it was", () => {
    expect(framed("w", FROM, ROOM, -100, 0)).toEqual({ ...FROM, left: 500, width: 400 });
  });

  it("grows from a corner both ways at once", () => {
    expect(framed("nw", FROM, ROOM, -100, -50)).toEqual({ left: 500, top: 50, width: 400, height: 450 });
  });

  it("never grows past the picture", () => {
    expect(framed("se", FROM, ROOM, 500, 500)).toEqual({ ...FROM, width: 400, height: 500 });
  });

  it("never shrinks past the smallest it reads at", () => {
    expect(framed("n", FROM, ROOM, 0, 1000)).toEqual({
      ...FROM,
      top: FROM.top + FROM.height - SMALLEST.height,
      height: SMALLEST.height,
    });
    expect(framed("e", FROM, ROOM, -1000, 0)).toEqual({ ...FROM, width: SMALLEST.width });
  });
});
