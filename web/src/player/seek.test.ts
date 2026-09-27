/*
 * Where the little picture over the bar stands, and how wide it is drawn.
 */

import { describe, expect, it } from "vitest";
import { previewPlace } from "./seek";

describe("the little picture over the bar", () => {
  it("stands over the hand in the middle of the bar", () => {
    expect(previewPlace(160, 0.5, 1000)).toEqual({ across: 160, left: 500 });
  });

  it("stays whole inside the bar at both ends", () => {
    expect(previewPlace(160, 0, 1000).left).toBe(80);
    expect(previewPlace(160, 1, 1000).left).toBe(920);
  });

  it("is never wider than the bar", () => {
    expect(previewPlace(400, 0.9, 300)).toEqual({ across: 300, left: 150 });
  });

  it("keeps the size asked for before the bar has been measured", () => {
    expect(previewPlace(160, 0.5, 0).across).toBe(160);
  });
});
