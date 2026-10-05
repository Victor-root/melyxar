import { describe, expect, it } from "vitest";
import {
  doingsAround,
  eventParts,
  frameStats,
  framesInView,
  framesLost,
  framesWhile,
  heldFor,
  heldUpByTheMainThread,
  latestBefore,
  passesOf,
  percentile,
  slowMoments,
} from "./measure-report";

const at = (points: [number, number][]) => points.map(([time, top]) => ({ at: time, top }));

describe("passesOf", () => {
  it("cuts a scroll down, back up and down again into three passes", () => {
    const passes = passesOf(
      at([
        [0, 0],
        [100, 400],
        [200, 3000],
        [300, 2980],
        [400, 1500],
        [500, 0],
        [600, 1200],
        [700, 3000],
      ]),
    );
    expect(passes.map((pass) => [pass.direction, pass.startTop, pass.endTop])).toEqual([
      ["down", 0, 3000],
      ["up", 3000, 0],
      ["down", 0, 3000],
    ]);
  });

  it("ends a pass at a long stillness, and ignores a hand settling", () => {
    const passes = passesOf(
      at([
        [0, 0],
        [100, 900],
        [5000, 930],
        [5100, 1900],
      ]),
    );
    expect(passes.map((pass) => [pass.startTop, pass.endTop])).toEqual([
      [0, 900],
      [930, 1900],
    ]);
  });
});

describe("frames", () => {
  it("counts the frames a gap let go by on a screen of a given rate", () => {
    expect(framesLost(16.7, 16.7)).toBe(0);
    expect(framesLost(50, 16.7)).toBe(2);
    expect(framesLost(21, 6.94)).toBe(2);
  });

  it("sums a list of gaps", () => {
    const stats = frameStats([16, 17, 16, 50, 17], 16.7);
    expect(stats).toMatchObject({ frames: 5, lost: 2, stalls: 1, worst: 50 });
    expect(percentile([1, 2, 3, 4], 0.5)).toBe(2);
  });
});

describe("the main thread's share of a frame", () => {
  const drawn = (before: number, painting?: number) => ({ at: 0, gap: 16.7, before, painting });

  it("adds what came before the recorder's turn to what came after", () => {
    expect(heldFor(drawn(2, 5))).toBe(7);
    expect(heldFor(drawn(2))).toBe(2);
  });

  it("blames a late frame on the main thread only when the one before left no room", () => {
    expect(heldUpByTheMainThread(drawn(3, 12), 16.7)).toBe(true);
    expect(heldUpByTheMainThread(drawn(1, 4), 16.7)).toBe(false);
    expect(heldUpByTheMainThread(undefined, 16.7)).toBe(false);
  });
});

describe("the frames drawn while something moved", () => {
  const drawn = (at: number) => ({ at, gap: 16.7, before: 1 });

  it("keeps those shortly after a move and lets the rest go", () => {
    const frames = [100, 120, 300, 320, 520, 700].map(drawn);
    expect(framesWhile(frames, [110, 310], 150).map((frame) => frame.at)).toEqual([120, 320]);
    expect(framesWhile(frames, [], 150)).toEqual([]);
  });
});

describe("the frames drawn while the page was in view", () => {
  const drawn = (at: number, gap: number) => ({ at, gap, before: 1 });

  it("drops the frame that came back from a tab in the background", () => {
    const frames = [drawn(100, 16), drawn(116, 16), drawn(9000, 8884), drawn(9016, 16)];
    expect(framesInView(frames, [{ from: 200, to: 8990 }]).map((frame) => frame.at)).toEqual([
      100, 116, 9016,
    ]);
    expect(framesInView(frames, [])).toEqual(frames);
  });
});

describe("latestBefore", () => {
  const entries = [{ at: 10 }, { at: 20 }, { at: 30 }];

  it("finds the entry at or before a moment, and none before the first", () => {
    expect(latestBefore(entries, 25)).toBe(entries[1]);
    expect(latestBefore(entries, 30)).toBe(entries[2]);
    expect(latestBefore(entries, 99)).toBe(entries[2]);
    expect(latestBefore(entries, 5)).toBeNull();
    expect(latestBefore([], 5)).toBeNull();
  });
});

describe("doingsAround", () => {
  const doings = [
    { at: 50, what: "over div.card" },
    { at: 200, what: "transition transform on div.card" },
    { at: 210, what: "over div.card" },
    { at: 230, what: "click button.header-icon" },
    { at: 900, what: "key in input.search" },
  ];

  it("lists each doing once, within reach of the frame and no later than it", () => {
    expect(doingsAround(doings, 220, 240, 30, 5)).toEqual([
      "transition transform on div.card",
      "over div.card",
      "click button.header-icon",
    ]);
    expect(doingsAround(doings, 500, 600, 30, 5)).toEqual([]);
  });

  it("keeps the latest ones when there are too many", () => {
    expect(doingsAround(doings, 0, 1000, 0, 2)).toEqual([
      "click button.header-icon",
      "key in input.search",
    ]);
  });
});

describe("eventParts", () => {
  it("splits an event into the wait, the handlers and the drawing", () => {
    expect(
      eventParts({ startTime: 100, duration: 64, processingStart: 112, processingEnd: 140 }),
    ).toEqual({ waited: 12, handling: 28, drawing: 24 });
  });

  it("never reads a negative part", () => {
    expect(
      eventParts({ startTime: 100, duration: 8, processingStart: 100, processingEnd: 120 }),
    ).toEqual({ waited: 0, handling: 20, drawing: 0 });
  });
});

describe("slowMoments", () => {
  const event = (
    name: string,
    what: string,
    startTime: number,
    duration: number,
    handled = 0,
  ) => ({
    name,
    what,
    startTime,
    duration,
    processingStart: startTime,
    processingEnd: startTime + handled,
  });

  it("makes one moment of the events one hover waited for the same frame", () => {
    const moments = slowMoments([
      event("pointerover", "div.card", 1000, 64),
      event("mouseover", "div.card", 1001, 64),
      event("pointerenter", "div.card", 1001, 64),
    ]);
    expect(moments).toHaveLength(1);
    expect(moments[0]).toMatchObject({ what: "div.card", duration: 64 });
    expect(moments[0].name).toBe("pointerover, mouseover, pointerenter");
  });

  it("keeps apart other elements, other frames and other moments", () => {
    const moments = slowMoments([
      event("pointerdown", "button.play", 1000, 32),
      event("click", "button.play", 1090, 128),
      event("pointerover", "div.card", 1000, 32),
      event("pointerover", "div.card", 5000, 32),
    ]);
    expect(moments.map((one) => [one.name, one.what])).toEqual([
      ["pointerdown", "button.play"],
      ["pointerover", "div.card"],
      ["click", "button.play"],
      ["pointerover", "div.card"],
    ]);
  });

  it("times a movement by its first event, which waited the longest", () => {
    const moments = slowMoments([
      event("pointerdown", "button.play", 1000, 40),
      event("pointerup", "button.play", 1010, 32, 25),
    ]);
    expect(moments).toHaveLength(1);
    expect(moments[0]).toMatchObject({ name: "pointerdown, pointerup", duration: 40 });
    expect(eventParts(moments[0])).toEqual({ waited: 0, handling: 35, drawing: 5 });
  });
});
