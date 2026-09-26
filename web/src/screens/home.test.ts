/*
 * The sections of the home page in the order a viewer chose, as the page
 * lays them out.
 */

import { describe, expect, it } from "vitest";
import { kindOfSection, laidOut, sectionsOnOffer } from "./home";

describe("the sections of the home page", () => {
  it("pairs the two rows of what is under way when they follow each other", () => {
    expect(laidOut(["band", "carry_on", "up_next", "newest:movies", "recently_added"])).toEqual([
      "band",
      ["carry_on", "up_next"],
      "newest:movies",
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

describe("the rows of each kind of library", () => {
  it("say which kind they show, and only they do", () => {
    expect(kindOfSection("newest:home_media")).toBe("home_media");
    expect(kindOfSection("recently_added")).toBeUndefined();
  });

  it("are offered only for the kinds this account holds", () => {
    expect(
      sectionsOnOffer(["band", "newest:movies", "newest:anime", "newest:music", "recently_added"], [
        "movies",
      ]),
    ).toEqual(["band", "newest:movies", "recently_added"]);
  });
});
