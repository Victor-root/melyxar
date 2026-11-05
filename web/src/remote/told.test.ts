import { describe, expect, it } from "vitest";
import type { Song } from "../music/api";
import { gathered, toldQueue } from "./told";

const song = (id: string) => ({ id }) as Song;

describe("what this device says of itself", () => {
  it("says the film's volume while there is one, and the music's queue all the same", () => {
    const queue = { songs: [song("a")], at: 0, shuffle: false, repeat: "off" };
    expect(gathered({ music: { queue, volume: 0.2, muted: false }, film: { volume: 0.9, muted: true } })).toEqual({
      volume: 0.9,
      muted: true,
      queue,
    });
    expect(gathered({ music: null, film: null })).toEqual({ volume: null, muted: null, queue: null });
  });

  it("says the queue in the order it plays, where it stands in that order", () => {
    const queue = { songs: [song("a"), song("b"), song("c")], order: [2, 0, 1], at: 1, shuffle: true, repeat: "all" as const };
    expect(toldQueue(queue)).toEqual({ songs: [song("c"), song("a"), song("b")], at: 1, shuffle: true, repeat: "all" });
    expect(toldQueue({ ...queue, songs: [], order: [] })).toBeNull();
  });
});
