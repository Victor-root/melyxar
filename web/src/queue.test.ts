import { describe, expect, it } from "vitest";
import { whatComesNext } from "./queue";

describe("whatComesNext", () => {
  const queue = { ids: ["film-a", "series-b", "film-c"], at: 0 };

  it("follows a work the queue named with the next one", () => {
    expect(whatComesNext(queue, "film-a", null)).toBe("series-b");
    expect(whatComesNext({ ...queue, at: 2 }, "film-c", null)).toBe(null);
  });

  it("lets a series in the queue carry on with its episodes, then moves on", () => {
    const inSeries = { ...queue, at: 1 };
    expect(whatComesNext(inSeries, "episode-2", "episode-3")).toBe("episode-3");
    expect(whatComesNext(inSeries, "episode-9", null)).toBe("film-c");
  });

  it("without a queue, only an episode has something to follow it", () => {
    expect(whatComesNext(null, "episode-2", "episode-3")).toBe("episode-3");
    expect(whatComesNext(null, "film-a", null)).toBe(null);
  });
});
