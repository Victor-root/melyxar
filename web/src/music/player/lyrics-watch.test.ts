import { describe, expect, it } from "vitest";
import { describeLines, lookAt, whyLit } from "./lyrics-watch";

describe("what is odd about the lines handed over", () => {
  it("counts the lines out of their order and the ones with nothing in them", () => {
    const lines = [1000, 3000, 2000, 4000, 4000].map((at_ms, index) => ({ at_ms, text: index === 3 ? " " : "words" }));
    expect(describeLines(lines)).toEqual({ out_of_order: 1, empty_lines: 1, first_at_ms: 1000, last_at_ms: 4000 });
    expect(describeLines([])).toEqual({ out_of_order: 0, empty_lines: 0, first_at_ms: null, last_at_ms: null });
  });
});

describe("why a line was lit", () => {
  it("is the first, the next one as the song went on, or any other after a jump", () => {
    expect(whyLit(null, 0)).toBe("first");
    expect(whyLit(4, 5)).toBe("played");
    expect(whyLit(4, 9)).toBe("after_a_jump");
    expect(whyLit(4, 2)).toBe("after_a_jump");
  });
});

describe("a look at the lines against the clock", () => {
  const fresh = { mismatches: 0, stills: 0, position: null as number | null };

  it("calls the line lit wrong only when it was wrong at two looks running", () => {
    const first = lookAt({ expected: 5, shown: 3, position: 20, playing: true }, fresh);
    expect(first.lineNotLit).toBe(false);
    const second = lookAt({ expected: 5, shown: 3, position: 22, playing: true }, first);
    expect(second.lineNotLit).toBe(true);
    const mended = lookAt({ expected: 6, shown: 6, position: 24, playing: true }, second);
    expect(mended.mismatches).toBe(0);
  });

  it("calls the clock stopped only when a song that plays has not moved at two looks running", () => {
    const first = lookAt({ expected: 1, shown: 1, position: 20, playing: true }, { ...fresh, position: 20 });
    expect(first.clockStill).toBe(false);
    const second = lookAt({ expected: 1, shown: 1, position: 20, playing: true }, first);
    expect(second.clockStill).toBe(true);
    const paused = lookAt({ expected: 1, shown: 1, position: 20, playing: false }, second);
    expect(paused.stills).toBe(0);
  });
});
