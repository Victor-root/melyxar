import { describe, expect, it } from "vitest";
import type { LyricsOffer } from "./api";
import { lengthGap, offersMissingLines } from "./lyrics-offers";

const offer = (id: number, gap: number | null, synced = true): LyricsOffer => ({
  id,
  artist: "A",
  title: "T",
  album: null,
  duration_ms: 166_000,
  synced,
  plain: true,
  instrumental: false,
  synced_lines: 40,
  longest_gap_seconds: gap,
});

describe("lengthGap", () => {
  it("says how much longer or shorter the offer is than the file", () => {
    expect(lengthGap(offer(1, null), 165_400)).toBe(600);
    expect(lengthGap(offer(1, null), 167_000)).toBe(-1_000);
  });

  it("says nothing when either length is unknown", () => {
    expect(lengthGap(offer(1, null), null)).toBeNull();
    expect(lengthGap({ ...offer(1, null), duration_ms: null }, 166_000)).toBeNull();
  });
});

describe("offersMissingLines", () => {
  it("says nothing when every version has the same long break: it is the song's own", () => {
    expect(offersMissingLines([offer(1, 39), offer(2, 39), offer(3, 40)]).size).toBe(0);
  });

  it("flags the versions whose stretch is clearly longer than the best one", () => {
    const flagged = offersMissingLines([offer(1, 12), offer(2, 39), offer(3, 20), offer(4, 22)]);
    expect([...flagged].sort()).toEqual([2, 4]);
  });

  it("has nothing to compare a lone version with", () => {
    expect(offersMissingLines([offer(1, 39)]).size).toBe(0);
    expect(offersMissingLines([offer(1, 39), offer(2, null), offer(3, 5, false)]).size).toBe(0);
  });
});
