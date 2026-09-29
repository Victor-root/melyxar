import { describe, expect, it } from "vitest";
import type { Song } from "../api";
import {
  afterTheEnd,
  backward,
  current,
  forward,
  jumpTo,
  nextRepeat,
  playLast,
  playNext,
  playing,
  upNext,
  withShuffle,
  without,
} from "./queue";

function song(title: string): Song {
  return { id: title, title, artists: [], album: null, track: null, disc: null, year: null, seconds: 200, source: null, cover: [] };
}

const ABCD = ["a", "b", "c", "d"].map(song);
const titles = (songs: Song[]) => songs.map((one) => one.title);
/** Always draws the last place, which reverses what it is given. */
const lastEachTime = () => 0.999;

describe("a queue played in order", () => {
  it("starts on the song chosen and plays on to the end", () => {
    let queue = playing(ABCD, 1, false, "off");
    expect(current(queue)?.title).toBe("b");
    expect(titles(upNext(queue))).toEqual(["c", "d"]);
    queue = forward(queue)!;
    queue = afterTheEnd(queue)!;
    expect(current(queue)?.title).toBe("d");
    expect(afterTheEnd(queue)).toBeNull();
  });

  it("goes round again under repeat all, and stays on one song under repeat one", () => {
    const all = playing(ABCD, 3, false, "all");
    expect(current(afterTheEnd(all)!)?.title).toBe("a");
    const one = playing(ABCD, 2, false, "one");
    expect(current(afterTheEnd(one)!)?.title).toBe("c");
    expect(current(forward(one)!)?.title).toBe("d");
  });

  it("goes back to the start of a song a few seconds in, and to the one before otherwise", () => {
    const queue = playing(ABCD, 2, false, "off");
    expect(backward(queue, 10)).toEqual({ queue, restart: true });
    expect(current(backward(queue, 1).queue)?.title).toBe("b");
    const first = playing(ABCD, 0, false, "off");
    expect(backward(first, 1)).toEqual({ queue: first, restart: true });
    const round = playing(ABCD, 0, false, "all");
    expect(current(backward(round, 1).queue)?.title).toBe("d");
  });

  it("repeats off, all, one, and off again", () => {
    expect(nextRepeat("off")).toBe("all");
    expect(nextRepeat("all")).toBe("one");
    expect(nextRepeat("one")).toBe("off");
  });
});

describe("a shuffled queue", () => {
  it("plays the song chosen first and every other one once", () => {
    const queue = playing(ABCD, 2, true, "off", lastEachTime);
    expect(current(queue)?.title).toBe("c");
    const rest = titles(upNext(queue));
    expect([...rest].sort()).toEqual(["a", "b", "d"]);
  });

  it("keeps the song playing when shuffle is turned on or off", () => {
    const inOrder = playing(ABCD, 1, false, "off");
    const shuffled = withShuffle(inOrder, true, lastEachTime);
    expect(current(shuffled)?.title).toBe("b");
    expect(shuffled.at).toBe(0);
    const back = withShuffle(shuffled, false);
    expect(current(back)?.title).toBe("b");
    expect(titles(upNext(back))).toEqual(["c", "d"]);
  });
});

describe("changing what is to come", () => {
  it("puts songs right after the one playing, or at the end", () => {
    const queue = playing(ABCD, 1, false, "off");
    expect(titles(upNext(playNext(queue, [song("x")])))).toEqual(["x", "c", "d"]);
    expect(titles(upNext(playLast(queue, [song("x")])))).toEqual(["c", "d", "x"]);
    expect(current(playNext(playing([], 0, false, "off"), [song("x")]))?.title).toBe("x");
  });

  it("takes out a song still to come, never one already played or playing", () => {
    const queue = playing(ABCD, 1, false, "off");
    expect(titles(upNext(without(queue, 3)))).toEqual(["c"]);
    expect(without(queue, 1)).toBe(queue);
    expect(without(queue, 0)).toBe(queue);
  });

  it("jumps to any song of the queue, and nowhere outside it", () => {
    const queue = playing(ABCD, 0, false, "off");
    expect(current(jumpTo(queue, 3))?.title).toBe("d");
    expect(jumpTo(queue, 9)).toBe(queue);
  });
});
