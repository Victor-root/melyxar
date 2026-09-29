import { describe, expect, it } from "vitest";
import type { Song } from "../api";
import { aSong } from "../testing";
import { levelOf } from "./levelling";

function song(lufs: number | null, peak: number | null, album: number | null = null): Song {
  return aSong({ lufs, peak_dbfs: peak, album_lufs: album });
}

const db = (factor: number) => Math.round(20 * Math.log10(factor) * 100) / 100;

describe("how much a song is raised or lowered", () => {
  it("lowers a loud song to the target", () => {
    expect(db(levelOf(song(-8, -0.1), "track"))).toBe(-6);
  });

  it("raises a quiet song only as far as its loudest moment allows", () => {
    expect(db(levelOf(song(-20, -10), "track"))).toBe(6);
    expect(db(levelOf(song(-20, -3), "track"))).toBe(2);
    expect(db(levelOf(song(-20, 0.5), "track"))).toBe(0);
    expect(db(levelOf(song(-20, null), "track"))).toBe(0);
  });

  it("levels by the album when asked, keeping the song's place in it", () => {
    expect(db(levelOf(song(-20, -0.1, -8), "album"))).toBe(-6);
    expect(db(levelOf(song(-8, -0.1, null), "album"))).toBe(-6);
  });

  it("leaves alone what was never measured, silence, and everything when off", () => {
    expect(levelOf(song(null, null), "track")).toBe(1);
    expect(levelOf(song(-70, null), "track")).toBe(1);
    expect(levelOf(song(-8, -0.1), "off")).toBe(1);
  });
});
