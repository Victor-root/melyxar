import { describe, expect, it } from "vitest";
import { keptWithin, soundBetween } from "./trim-cut";

const waveOf = (momentsASecond: number, levels: number[]) => ({ momentsASecond, levels: new Uint8Array(levels) });

describe("soundBetween", () => {
  it("leaves out the silence at either end, a breath kept on each side", () => {
    // Four moments a second: a second of hiss, two of sound, a second of silence.
    const wave = waveOf(4, [40, 51, 30, 0, 120, 200, 180, 150, 230, 90, 60, 52, 0, 0, 0, 0]);
    expect(soundBetween(wave, 4)).toEqual({ from: 0.85, to: 3.15 });
  });

  it("says nothing of a song silent throughout", () => {
    expect(soundBetween(waveOf(4, [0, 51, 0]), 0.75)).toBeNull();
  });

  it("never reaches past the song", () => {
    expect(soundBetween(waveOf(4, [230, 230, 230, 230]), 1)).toEqual({ from: 0, to: 1 });
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
