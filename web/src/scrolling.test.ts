import { describe, expect, it } from "vitest";
import { isThere, pageOf, remember } from "./scrolling";

describe("remember", () => {
  it("keeps the latest place of a page, most recent last", () => {
    let places = new Map();
    places = remember(places, "a", { top: 10, rows: [] }, 3);
    places = remember(places, "b", { top: 20, rows: [] }, 3);
    places = remember(places, "a", { top: 30, rows: [5] }, 3);
    expect(Array.from(places.keys())).toEqual(["b", "a"]);
    expect(places.get("a")).toEqual({ top: 30, rows: [5] });
  });

  it("forgets the oldest pages past the number kept", () => {
    let places = new Map();
    for (const page of ["a", "b", "c", "d"]) {
      places = remember(places, page, { top: 0, rows: [] }, 3);
    }
    expect(Array.from(places.keys())).toEqual(["b", "c", "d"]);
  });
});

describe("isThere", () => {
  it("is there within a pixel, rows included", () => {
    expect(isThere({ top: 500.4, rows: [0, 300] }, { top: 500, rows: [0, 300] })).toBe(true);
    expect(isThere({ top: 480, rows: [0, 300] }, { top: 500, rows: [0, 300] })).toBe(false);
  });

  it("is not there while a row it had is still missing", () => {
    expect(isThere({ top: 500, rows: [0] }, { top: 500, rows: [0, 300] })).toBe(false);
  });
});

describe("pageOf", () => {
  it("tells apart two pages the history gave the same place", () => {
    // The first page of a tab always has the same key, whatever it shows.
    const first = pageOf({ key: "default", pathname: "/", search: "" });
    const openedAfresh = pageOf({ key: "default", pathname: "/library/films", search: "" });
    expect(first).not.toEqual(openedAfresh);
  });

  it("names the same page the same way", () => {
    const at = { key: "k1", pathname: "/search", search: "?q=harbour" };
    expect(pageOf(at)).toEqual(pageOf({ ...at }));
  });
});
