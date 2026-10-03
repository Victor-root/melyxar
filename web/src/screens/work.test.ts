/*
 * Where a work is described elsewhere, in the order the page shows it.
 */

import { describe, expect, it } from "vitest";
import { linksOf } from "./work";

describe("linksOf", () => {
  it("puts the site most of the page comes from first", () => {
    const ids = [
      { provider: "imdb", id: "tt0000001" },
      { provider: "tmdb", id: "42" },
    ];
    expect(linksOf(ids, "movie").map((entry) => entry.provider)).toEqual(["tmdb", "imdb"]);
  });

  it("leaves out what cannot be linked to", () => {
    const ids = [
      { provider: "tvdb", id: "7" },
      { provider: "tmdb", id: "42" },
    ];
    expect(linksOf(ids, "movie").map((entry) => entry.provider)).toEqual(["tmdb"]);
    expect(linksOf(ids, "episode")).toEqual([]);
  });
});
