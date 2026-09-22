/*
 * The bar's own arithmetic: turning a scope into an address, and back.
 *
 * A scope is one value doing the work of two, a category or a single
 * library, and the address and the works endpoint each read it their own
 * way. Getting either wrong sends a search to the wrong shelf while looking
 * exactly like it asked for the right one.
 */

import { describe, expect, it } from "vitest";
import { scopeToBrowse, searchAddress } from "./header";

describe("searchAddress", () => {
  it("carries the words alone when nothing narrows them", () => {
    expect(searchAddress("dune", "")).toBe("/search?search=dune");
  });

  it("carries the scope alongside the words when there is one", () => {
    expect(searchAddress("dune", "kind:movies")).toBe(
      "/search?search=dune&in=kind%3Amovies",
    );
  });

  it("escapes what the words themselves need escaping", () => {
    expect(searchAddress("l'associé & fils", "")).toBe(
      "/search?search=l%27associ%C3%A9+%26+fils",
    );
  });
});

describe("scopeToBrowse", () => {
  it("narrows by nothing when the scope is everywhere", () => {
    expect(scopeToBrowse("")).toEqual({});
  });

  it("narrows by kind for a category", () => {
    expect(scopeToBrowse("kind:series")).toEqual({ kind: "series" });
  });

  it("narrows by library for a single one, kind left out", () => {
    expect(scopeToBrowse("library:abc123")).toEqual({ library: "abc123" });
  });
});
