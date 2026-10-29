import { describe, expect, it } from "vitest";
import { barsOf, clockOf, paged, panned, showing, zoomed } from "./wave-stretch";

const waveOf = (momentsASecond: number, levels: number[]) => ({ momentsASecond, levels: new Uint8Array(levels) });

describe("zoomed", () => {
  it("keeps the moment it zooms around where it was on the bar", () => {
    expect(zoomed({ from: 0, to: 100 }, 100, 4, 20)).toEqual({ from: 15, to: 40 });
  });

  it("stops at the closest stretch and at the whole song", () => {
    expect(zoomed({ from: 10, to: 12 }, 100, 8, 11)).toEqual({ from: 10.25, to: 11.75 });
    expect(zoomed({ from: 10, to: 60 }, 100, 0.1, 30)).toEqual({ from: 0, to: 100 });
  });

  it("never shows past either end of the song", () => {
    expect(zoomed({ from: 0, to: 100 }, 100, 2, 0)).toEqual({ from: 0, to: 50 });
    expect(zoomed({ from: 50, to: 100 }, 100, 0.8, 100)).toEqual({ from: 37.5, to: 100 });
  });
});

describe("panned", () => {
  it("moves along and stops at either end", () => {
    expect(panned({ from: 10, to: 20 }, 100, 5)).toEqual({ from: 15, to: 25 });
    expect(panned({ from: 10, to: 20 }, 100, -15)).toEqual({ from: 0, to: 10 });
    expect(panned({ from: 80, to: 90 }, 100, 30)).toEqual({ from: 90, to: 100 });
  });
});

describe("showing", () => {
  it("is left alone when the moment is on it", () => {
    const shown = { from: 10, to: 20 };
    expect(showing(shown, 100, 15)).toBe(shown);
  });

  it("moves just enough for the moment to be on it", () => {
    expect(showing({ from: 10, to: 20 }, 100, 25)).toEqual({ from: 16, to: 26 });
    expect(showing({ from: 10, to: 20 }, 100, 5)).toEqual({ from: 4, to: 14 });
  });
});

describe("paged", () => {
  it("is left alone while the moment is on it", () => {
    const shown = { from: 10, to: 20 };
    expect(paged(shown, 100, 18.9)).toBe(shown);
  });

  it("turns over once the moment runs off it, either way", () => {
    expect(paged({ from: 10, to: 20 }, 100, 19.5)).toEqual({ from: 18.5, to: 28.5 });
    expect(paged({ from: 10, to: 20 }, 100, 4)).toEqual({ from: 3, to: 13 });
    expect(paged({ from: 10, to: 20 }, 100, 99)).toEqual({ from: 90, to: 100 });
  });
});

describe("barsOf", () => {
  it("draws each bar as the loudest of the moments it covers", () => {
    expect(barsOf(waveOf(4, [0, 51, 255, 102]), { from: 0, to: 1 }, 2)).toEqual([0.2, 1]);
  });

  it("draws the moment a bar falls on when zoomed in closer", () => {
    expect(barsOf(waveOf(4, [0, 51, 255, 102]), { from: 0.5, to: 1 }, 4)).toEqual([1, 1, 0.4, 0.4]);
  });

  it("draws nothing past the end of the wave", () => {
    expect(barsOf(waveOf(4, [255]), { from: 0, to: 1 }, 2)).toEqual([1, 0]);
  });
});

describe("clockOf", () => {
  it("reads as minutes, seconds and tenths", () => {
    expect(clockOf(0)).toBe("0:00.0");
    expect(clockOf(65.34)).toBe("1:05.3");
    expect(clockOf(599.96)).toBe("10:00.0");
  });
});
