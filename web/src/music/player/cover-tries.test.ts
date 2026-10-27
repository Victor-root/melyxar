import { describe, expect, it } from "vitest";

import { COVER_AGAIN_AFTER_MS, COVER_PATIENCE_MS, COVER_TRIES, again, coverAt, drawn, waitBeforeAgain } from "./cover-tries";
import type { CoverTry } from "./cover-tries";

const NONE: CoverTry = { src: null, times: 0, state: "loading" };

describe("cover tries", () => {
  it("starts over for another cover", () => {
    const failed: CoverTry = { src: "/a", times: 2, state: "failed" };
    expect(coverAt(failed, "/b")).toEqual({ src: "/b", times: 0, state: "loading" });
    expect(coverAt(failed, "/a")).toBe(failed);
  });

  it("waits longer on a cover still coming than on one that failed", () => {
    const loading = coverAt(NONE, "/a");
    expect(waitBeforeAgain(loading)).toBe(COVER_PATIENCE_MS);
    expect(waitBeforeAgain({ ...loading, state: "failed" })).toBe(COVER_AGAIN_AFTER_MS);
    expect(waitBeforeAgain({ ...loading, state: "loaded" })).toBeNull();
    expect(waitBeforeAgain(NONE)).toBeNull();
  });

  it("gives up after the last try", () => {
    let cover = coverAt(NONE, "/a");
    for (let tried = 1; tried < COVER_TRIES; tried += 1) {
      cover = again({ ...cover, state: "failed" });
      expect(cover).toEqual({ src: "/a", times: tried, state: "loading" });
    }
    cover = again(cover);
    expect(cover.state).toBe("gave_up");
    expect(waitBeforeAgain(cover)).toBeNull();
  });

  it("draws nothing for a cover that failed, until it is asked for again", () => {
    const cover = coverAt(NONE, "/a");
    expect(drawn(cover)).toBe(true);
    expect(drawn({ ...cover, state: "failed" })).toBe(false);
    expect(drawn({ ...cover, state: "gave_up" })).toBe(false);
    expect(drawn(NONE)).toBe(false);
  });
});
