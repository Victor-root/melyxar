import { describe, expect, it, vi } from "vitest";
import { obeyFilm } from "./steered";
import type { SteeredFilm } from "./steered";

function film(playing: boolean): SteeredFilm {
  return {
    playing,
    playOrPause: vi.fn(),
    goTo: vi.fn(),
    stepBy: vi.fn(),
    leave: vi.fn(),
    setLoudness: vi.fn(),
    setMuted: vi.fn(),
    audioId: "a1",
    subtitleId: "s1",
    choose: vi.fn(),
  };
}

describe("the film steered from another device", () => {
  it("pauses a film that plays and leaves one already paused as it is", () => {
    const playing = film(true);
    const paused = film(false);
    obeyFilm({ do: "pause" }, playing);
    obeyFilm({ do: "pause" }, paused);
    expect(playing.playOrPause).toHaveBeenCalledOnce();
    expect(paused.playOrPause).not.toHaveBeenCalled();
    obeyFilm({ do: "resume" }, paused);
    expect(paused.playOrPause).toHaveBeenCalledOnce();
  });

  it("changes one track and keeps the other", () => {
    const here = film(true);
    obeyFilm({ do: "subtitle", track: null }, here);
    obeyFilm({ do: "audio", track: "a2" }, here);
    expect(here.choose).toHaveBeenNthCalledWith(1, "a1", null);
    expect(here.choose).toHaveBeenNthCalledWith(2, "a2", "s1");
  });

  it("says it could not go to an episode that is not there", () => {
    expect(obeyFilm({ do: "next" }, film(true))).toBe(false);
  });
});
