import { describe, expect, it } from "vitest";
import { mirroredHeights, radialWave } from "./ring-wave";

/** Every point the path goes through, from its numbers taken two by two. */
function pointsOf(path: string): { x: number; y: number }[] {
  const numbers = path.match(/-?\d+(\.\d+)?/g)?.map(Number) ?? [];
  const points = [];
  for (let at = 0; at + 1 < numbers.length; at += 2) {
    points.push({ x: numbers[at], y: numbers[at + 1] });
  }
  return points;
}

describe("radialWave", () => {
  it("is one closed curve with the same commands whatever the heights", () => {
    const low = radialWave([0, 0, 0, 0], 0.7);
    const high = radialWave([1, 0.5, 1, 0.5], 0.7);
    expect(low.endsWith("Z")).toBe(true);
    expect(low.replace(/[-\d. ]/g, "")).toBe(high.replace(/[-\d. ]/g, ""));
  });

  it("stands between the disc and the edge of its box", () => {
    for (const point of pointsOf(radialWave([0, 1, 0.3, 2, -1, 0.7], 0.7))) {
      const away = Math.hypot(point.x, point.y);
      expect(away).toBeLessThanOrEqual(1.001);
      expect(away).toBeGreaterThanOrEqual(0.6);
    }
  });

  it("puts its first spike at the top", () => {
    const tips = pointsOf(radialWave([1, 0, 0, 0], 0.5));
    expect(tips[1]).toEqual({ x: 0, y: -1 });
  });
});

describe("mirroredHeights", () => {
  it("mirrors each shape round its middle, and is the same each time", () => {
    const shapes = mirroredHeights(8, 3);
    expect(shapes).toHaveLength(3);
    for (const shape of shapes) {
      expect(shape).toHaveLength(8);
      for (let at = 1; at < 8; at += 1) {
        expect(shape[at]).toBe(shape[8 - at]);
      }
    }
    expect(mirroredHeights(8, 3)).toEqual(shapes);
  });
});
