/*
 * What a card or a page offers. A slip here puts a play button on a photo,
 * which opens a player on a still picture, or offers to look up a family
 * video in a catalogue of films.
 */

import { describe, expect, it } from "vitest";
import { isCatalogued, isNamed, playsOnItsOwn } from "./works";

describe("what a work offers", () => {
  it("plays what has a file and is meant to be played", () => {
    expect(playsOnItsOwn({ source: "s", kind: "movie" })).toBe(true);
    expect(playsOnItsOwn({ source: "s", kind: "video" })).toBe(true);
    expect(playsOnItsOwn({ source: "s", kind: "photo" })).toBe(false);
    expect(playsOnItsOwn({ source: null, kind: "series" })).toBe(true);
    expect(playsOnItsOwn({ source: null, kind: "folder" })).toBe(false);
    expect(playsOnItsOwn({ source: null, kind: "movie" })).toBe(false);
  });

  it("counts a file of one's own as named, and as in no catalogue", () => {
    expect(isNamed("own")).toBe(true);
    expect(isNamed("manual")).toBe(true);
    expect(isNamed("pending")).toBe(false);
    expect(isNamed("unidentified")).toBe(false);
    expect(isCatalogued("own")).toBe(false);
    expect(isCatalogued("unidentified")).toBe(true);
  });
});
