import { describe, expect, it } from "vitest";
import { clockOf, keptWithin, loudnessOf, soundBetween } from "./trim-cut";

describe("loudnessOf", () => {
  it("takes the loudest band of each moment", () => {
    const spectrum = { bands: 2, framesASecond: 4, levels: new Uint8Array([0, 0, 51, 255, 102, 0]) };
    expect(loudnessOf(spectrum)).toEqual([0, 1, 0.4]);
  });
});

describe("soundBetween", () => {
  it("leaves out the silence at either end, a breath kept on each side", () => {
    // Four moments a second: a second of silence, two of sound, a second of silence.
    const loudness = [0, 0, 0, 0, 0.5, 0.8, 0.7, 0.6, 0.9, 0.4, 0.3, 0.2, 0, 0, 0, 0];
    expect(soundBetween(loudness, 4, 4)).toEqual({ from: 0.85, to: 3.15 });
  });

  it("says nothing of a song silent throughout", () => {
    expect(soundBetween([0, 0.01, 0], 4, 0.75)).toBeNull();
  });

  it("never reaches past the song", () => {
    expect(soundBetween([0.9, 0.9, 0.9, 0.9], 4, 1)).toEqual({ from: 0, to: 1 });
  });
});

describe("keptWithin", () => {
  it("brings a cut inside the song", () => {
    expect(keptWithin(-2, 300, 200)).toEqual({ from: 0, to: 200 });
  });

  it("keeps a second at least, by moving the start", () => {
    expect(keptWithin(9.6, 10, 200)).toEqual({ from: 9, to: 10 });
  });
});

describe("clockOf", () => {
  it("reads as minutes, seconds and tenths", () => {
    expect(clockOf(0)).toBe("0:00.0");
    expect(clockOf(65.34)).toBe("1:05.3");
    expect(clockOf(599.96)).toBe("10:00.0");
  });
});
