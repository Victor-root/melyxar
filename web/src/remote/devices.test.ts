import { describe, expect, it } from "vitest";
import type { SteeredPlaying } from "../api";
import { positionNow } from "./devices";

function playing(changes: Partial<SteeredPlaying>): SteeredPlaying {
  return {
    work_id: "w",
    title: "Quiet Harbour",
    kind: "movie",
    series: null,
    season: null,
    episode: null,
    artist: null,
    album: null,
    picture: null,
    position_seconds: 100,
    duration_seconds: 120,
    paused: false,
    audio: [],
    subtitles: [],
    chosen_audio_id: null,
    chosen_subtitle_id: null,
    ...changes,
  };
}

describe("where what plays elsewhere has got to", () => {
  it("runs on from what was told while it plays", () => {
    expect(positionNow(playing({}), 1_000, 6_000)).toBe(105);
  });

  it("stands still while it is paused", () => {
    expect(positionNow(playing({ paused: true }), 1_000, 6_000)).toBe(100);
  });

  it("never runs past its end, and runs on with no end known", () => {
    expect(positionNow(playing({}), 0, 60_000)).toBe(120);
    expect(positionNow(playing({ duration_seconds: null }), 0, 60_000)).toBe(160);
  });
});
