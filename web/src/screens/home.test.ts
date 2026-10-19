/*
 * The sections of the home page in the order a viewer chose, as the page
 * lays them out.
 */

import { describe, expect, it } from "vitest";
import { laidOut, libraryOfSection } from "./home";

describe("the sections of the home page", () => {
  it("pairs the two rows of what is under way when they follow each other", () => {
    expect(laidOut(["band", "carry_on", "up_next", "newest:0199-films", "recently_added"])).toEqual([
      "band",
      ["carry_on", "up_next"],
      "newest:0199-films",
      "recently_added",
    ]);
    expect(laidOut(["up_next", "carry_on"])).toEqual([["up_next", "carry_on"]]);
  });

  it("lays them out apart when another section stands between them", () => {
    expect(laidOut(["carry_on", "band", "up_next"])).toEqual(["carry_on", "band", "up_next"]);
  });

  it("lays out nothing for a page with every section hidden", () => {
    expect(laidOut([])).toEqual([]);
  });
});

describe("the rows of each library", () => {
  it("say which library they show, and only they do", () => {
    expect(libraryOfSection("newest:0199-tests")).toBe("0199-tests");
    expect(libraryOfSection("recently_added")).toBeUndefined();
  });
});
