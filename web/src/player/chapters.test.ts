/*
 * Where a film stands among its chapters, and where the chapter buttons go.
 */

import { describe, expect, it } from "vitest";
import type { PlaybackChapter } from "../api";
import { chapterAt, chapterStep } from "./chapters";

const chapters: PlaybackChapter[] = [
  { at_second: 0, title: null },
  { at_second: 100, title: null },
  { at_second: 200, title: null },
];

describe("the chapter a moment falls inside", () => {
  it("is the last one started", () => {
    expect(chapterAt(chapters, 150)).toBe(1);
    expect(chapterAt(chapters, 250)).toBe(2);
  });

  it("counts a film landed a hair before a chapter as inside it", () => {
    expect(chapterAt(chapters, 99.9)).toBe(1);
    expect(chapterAt(chapters, 99.5)).toBe(0);
  });

  it("is none before the first", () => {
    expect(chapterAt([{ at_second: 30, title: null }], 10)).toBe(-1);
  });
});

describe("where a chapter button takes the film", () => {
  it("goes forward to the next chapter, and to the end after the last", () => {
    expect(chapterStep(chapters, 150, 300, false)).toEqual({ to: 200, possible: true });
    expect(chapterStep(chapters, 250, 300, false)).toEqual({ to: 300, possible: false });
  });

  it("goes back to the start of this chapter", () => {
    expect(chapterStep(chapters, 150, 300, true)).toEqual({ to: 100, possible: true });
  });

  it("goes back to the one before when this one has only just begun", () => {
    expect(chapterStep(chapters, 101, 300, true)).toEqual({ to: 0, possible: true });
  });

  it("goes back to the very start before the first chapter, and nowhere at it", () => {
    const late: PlaybackChapter[] = [{ at_second: 60, title: null }];
    expect(chapterStep(late, 30, 300, true)).toEqual({ to: 0, possible: true });
    expect(chapterStep(late, 1, 300, true)).toEqual({ to: 0, possible: false });
  });
});
