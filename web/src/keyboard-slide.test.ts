import { describe, expect, it } from "vitest";
import { keyboardAt, keyboardRise } from "./keyboard-slide";

const slide = { from: 0, to: 300, duration: 100, course: [0, 0.5, 1], startedAt: 1000 };

describe("keyboardAt", () => {
  it("stands at either end outside the slide", () => {
    expect(keyboardAt(slide, 900)).toBe(0);
    expect(keyboardAt(slide, 1200)).toBe(300);
  });

  it("follows the course between its samples", () => {
    expect(keyboardAt(slide, 1050)).toBe(150);
    expect(keyboardAt(slide, 1025)).toBe(75);
  });

  it("goes down as well as up", () => {
    expect(keyboardAt({ ...slide, from: 300, to: 0 }, 1075)).toBe(75);
  });
});

describe("keyboardRise", () => {
  it("starts above the navigation bar", () => {
    expect(keyboardRise(10, 0, 24)).toBe(0);
    expect(keyboardRise(300, 0, 24)).toBe(276);
  });

  it("is nothing once the window has given up the room", () => {
    expect(keyboardRise(300, 300, 0)).toBe(0);
  });

  it("takes all the height back when the window is whole again and the bar is gone", () => {
    expect(keyboardRise(300, 0, 0)).toBe(300);
  });
});
