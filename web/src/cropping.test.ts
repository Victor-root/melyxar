import { describe, expect, it } from "vitest";
import { centred, held, moved, squareShown, zoomedTo } from "./cropping";

// A photo taken sideways: twice as wide as it is tall, in a frame of 200.
const [frame, width, height] = [200, 1000, 500];

describe("cropping", () => {
  it("starts on the middle, the short side filling the frame", () => {
    const start = centred(frame, width, height);
    expect(start).toEqual({ zoom: 1, x: -100, y: 0 });
    expect(squareShown(start, frame, width, height)).toEqual({ x: 250, y: 0, side: 500 });
  });

  it("never lets an empty corner show", () => {
    const start = centred(frame, width, height);
    expect(moved(start, 500, 50, frame, width, height)).toEqual({ zoom: 1, x: 0, y: 0 });
    expect(moved(start, -500, -50, frame, width, height)).toEqual({ zoom: 1, x: -200, y: 0 });
  });

  it("keeps the zoom between the whole picture and four times", () => {
    const start = centred(frame, width, height);
    expect(held({ ...start, zoom: 0.2 }, frame, width, height).zoom).toBe(1);
    expect(held({ ...start, zoom: 9 }, frame, width, height).zoom).toBe(4);
  });

  it("zooms around what is in the middle of the frame", () => {
    const start = centred(frame, width, height);
    const closer = zoomedTo(start, 2, frame, width, height);
    const shown = squareShown(closer, frame, width, height);
    expect(shown.side).toBe(250);
    // Still centred on the middle of the picture.
    expect(shown.x + shown.side / 2).toBe(500);
    expect(shown.y + shown.side / 2).toBe(250);
  });

  it("zoomed back out, pulls the picture over any corner it left bare", () => {
    const closer = zoomedTo(centred(frame, width, height), 3, frame, width, height);
    const pulled = moved(closer, -1000, -1000, frame, width, height);
    const back = zoomedTo(pulled, 1, frame, width, height);
    expect(back).toEqual({ zoom: 1, x: -200, y: 0 });
  });
});
