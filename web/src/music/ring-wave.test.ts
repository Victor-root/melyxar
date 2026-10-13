import { describe, expect, it } from "vitest";
import { ringWave } from "./ring-wave";

/** Every point the path goes through, from its numbers taken two by two. */
function pointsOf(path: string): { x: number; y: number }[] {
  const numbers = path.match(/-?\d+(\.\d+)?/g)?.map(Number) ?? [];
  const points = [];
  for (let at = 0; at + 1 < numbers.length; at += 2) {
    points.push({ x: numbers[at], y: numbers[at + 1] });
  }
  return points;
}

describe("ringWave", () => {
  it("is one closed curve", () => {
    const path = ringWave(5, 0.6);
    expect(path.startsWith("M")).toBe(true);
    expect(path.endsWith("Z")).toBe(true);
  });

  it("keeps its edge between the disc and the edge of its box", () => {
    for (const point of pointsOf(ringWave(6, 0.62, 0.8))) {
      const away = Math.hypot(point.x, point.y);
      expect(away).toBeLessThanOrEqual(1.001);
      expect(away).toBeGreaterThanOrEqual(0.55);
    }
  });

  it("rises and falls, rather than being a plain circle", () => {
    const aways = pointsOf(ringWave(5, 0.6)).map((point) => Math.hypot(point.x, point.y));
    expect(Math.max(...aways) - Math.min(...aways)).toBeGreaterThan(0.25);
  });
});
