import { describe, expect, it, vi } from "vitest";
import type { Song } from "../api";
import { obeyMusic } from "./remote";
import type { SteeredMusic } from "./remote";

function music(at = 0): SteeredMusic {
  return {
    play: vi.fn(),
    toggle: vi.fn(),
    pause: vi.fn(),
    resume: vi.fn(),
    stop: vi.fn(),
    next: vi.fn(),
    previous: vi.fn(),
    seek: vi.fn(),
    setVolume: vi.fn(),
    volume: vi.fn(() => 0.5),
    setMuted: vi.fn(),
    toggleShuffle: vi.fn(),
    cycleRepeat: vi.fn(),
    jump: vi.fn(),
    at: vi.fn(() => at),
  };
}

describe("the music steered from another device", () => {
  it("is left alone by a page that does not answer for it", () => {
    const here = music();
    expect(obeyMusic({ do: "toggle" }, here, false)).toBe(false);
    expect(here.toggle).not.toHaveBeenCalled();
  });

  it("starts on the page handed the songs, from the moment it was at", () => {
    const here = music();
    const songs = [{ id: "a" }, { id: "b" }] as Song[];
    expect(obeyMusic({ do: "play_songs", songs, at: 1, shuffle: false, from_seconds: 42 }, here, false)).toBe(true);
    expect(here.play).toHaveBeenCalledWith(songs, 1, false, 42);
  });

  it("steps from where the song has got to, never before its start", () => {
    const here = music(5);
    obeyMusic({ do: "step", seconds: 10 }, here, true);
    obeyMusic({ do: "step", seconds: -10 }, here, true);
    expect(vi.mocked(here.seek).mock.calls).toEqual([[15], [0]]);
  });

  it("leaves a film's tracks and a film started to the film", () => {
    const here = music();
    expect(obeyMusic({ do: "audio", track: "1" }, here, true)).toBe(false);
    expect(obeyMusic({ do: "play_work", work: "w", source: null }, here, true)).toBe(false);
  });
});
