import { describe, expect, it } from "vitest";
import type { Watched, WatchedDecision } from "../../api";
import { countedOf, episodeOf, positionNow, shareWatched } from "./playing";

function watched(changes: Partial<Watched> = {}): Watched {
  return {
    device: "d",
    user: "somebody",
    device_name: "a browser",
    browser: null,
    work_id: "w",
    title: "Quiet Harbour",
    kind: "movie",
    year: 2019,
    series: null,
    season: null,
    episode: null,
    picture: null,
    position_seconds: 0,
    duration_seconds: null,
    started_at: "2026-01-01T20:00:00Z",
    paused: false,
    stopping: false,
    producing: null,
    decision: null,
    ...changes,
  };
}

function decided(method: string, expensive: boolean): WatchedDecision {
  return {
    method,
    expensive,
    reasons: [],
    film: { container: null, size_bytes: 1, overall_bitrate: null, picture: null, sound: null },
    rebuild: null,
    card_way: null,
    card_reads_the_film: false,
    sound: "copy",
    subtitles: "none",
    tone_map: false,
  };
}

const t = (key: string, values?: Record<string, string | number>) =>
  key === "home.up_next.short" ? `S${values?.season}E${values?.episode}` : key;

describe("countedOf", () => {
  it("counts a repackaged film as reaching its viewer as it is", () => {
    const all = [
      watched({ decision: decided("direct_play", false) }),
      watched({ decision: decided("remux", false) }),
      watched({ decision: decided("full_transcode", true) }),
      watched({ decision: decided("transcode_audio", true) }),
    ];
    expect(countedOf(all)).toEqual({ playing: 4, rebuilt: 2, direct: 2 });
  });

  it("counts a film whose plan was never heard as playing and nothing else", () => {
    expect(countedOf([watched()])).toEqual({ playing: 1, rebuilt: 0, direct: 0 });
    expect(countedOf([])).toEqual({ playing: 0, rebuilt: 0, direct: 0 });
  });
});

describe("shareWatched", () => {
  it("is how far through the film its viewer is", () => {
    expect(shareWatched(watched({ duration_seconds: 6000 }), 1500)).toBe(0.25);
  });

  it("stays inside the film and says nothing of a film of unknown length", () => {
    expect(shareWatched(watched({ duration_seconds: 6000 }), 7000)).toBe(1);
    expect(shareWatched(watched(), 10)).toBeNull();
    expect(shareWatched(watched({ duration_seconds: 0 }), 10)).toBeNull();
  });
});

describe("positionNow", () => {
  it("runs the clock of a film playing on from when the server spoke", () => {
    const playing = watched({ position_seconds: 100, duration_seconds: 6000 });
    expect(positionNow(playing, 5_000, 12_500)).toBe(107.5);
  });

  it("holds a paused film where it stands, and never runs past the end", () => {
    expect(positionNow(watched({ position_seconds: 100, paused: true }), 0, 60_000)).toBe(100);
    expect(positionNow(watched({ position_seconds: 5995, duration_seconds: 6000 }), 0, 60_000)).toBe(6000);
    expect(positionNow(watched({ position_seconds: 50 }), 0, 60_000)).toBe(110);
  });
});

describe("episodeOf", () => {
  it("names the series and where the episode sits in it", () => {
    expect(episodeOf(watched({ series: "Lantern Street", season: 2, episode: 5 }), t)).toBe(
      "Lantern Street · S2E5",
    );
    expect(episodeOf(watched({ series: "Lantern Street" }), t)).toBe("Lantern Street");
    expect(episodeOf(watched(), t)).toBeNull();
  });
});
