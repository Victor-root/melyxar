import { describe, expect, it } from "vitest";
import { isThere, keepsTheSound, positionAfter } from "./tabs";
import type { Beat } from "./tabs";

function beat(changes: Partial<Beat> = {}): Beat {
  return { playing: true, waiting: false, position: 10, length: 200, loudness: { volume: 1, muted: false }, ...changes };
}

describe("positionAfter", () => {
  it("moves on with the time that has passed while it plays", () => {
    expect(positionAfter(beat(), 2500)).toBe(12);
  });

  it("stays where it was while paused", () => {
    expect(positionAfter(beat({ playing: false }), 5000)).toBe(10);
  });

  it("does not go past the end of the song", () => {
    expect(positionAfter(beat({ position: 199 }), 5000)).toBe(200);
  });

  it("goes on when the length is not known", () => {
    expect(positionAfter(beat({ length: 0 }), 3000)).toBe(13);
  });
});

describe("isThere", () => {
  it("is there shortly after it spoke and gone after a silence", () => {
    expect(isThere(1000, 2000)).toBe(true);
    expect(isThere(1000, 5000)).toBe(false);
  });
});

describe("keepsTheSound", () => {
  it("is decided the same way in both tabs", () => {
    expect(keepsTheSound("a", "b")).toBe(true);
    expect(keepsTheSound("b", "a")).toBe(false);
  });
});
