/*
 * Which posters a way in fans out.
 *
 * The rule looks like nothing until a library disagrees with it: a fan of
 * holes where the posters should be reads as an empty library, and a fan
 * that shuffles between two visits reads as a page that cannot make up its
 * mind. Neither shows up on screen as a fault, which is what this is here
 * for.
 */

import { describe, expect, it } from "vitest";
import type { Card, Picture } from "../api";
import { fanOf } from "./band";

/** A poster, at the one size this test cares about. */
const APICTURE: Picture[] = [{ url: "/pictures/one-200.webp", width: 200, height: 300 }];

/** A card with only what the fan reads of it. */
function work(id: string, poster: Picture[]): Card {
  return {
    id,
    title: id,
    year: null,
    runtime_minutes: null,
    rating: null,
    identification: "identified",
    identification_note: null,
    color: null,
    poster,
    wide: [],
  } as unknown as Card;
}

describe("fanOf", () => {
  it("takes the five newest, in the order they arrived", () => {
    const cards = ["a", "b", "c", "d", "e", "f", "g"].map((id) => work(id, APICTURE));
    expect(fanOf(cards).map((card) => card.id)).toEqual(["a", "b", "c", "d", "e"]);
  });

  it("passes over a work with no poster rather than fanning out a hole", () => {
    const cards = [
      work("newest-but-bare", []),
      work("b", APICTURE),
      work("c", APICTURE),
      work("d", APICTURE),
      work("e", APICTURE),
      work("f", APICTURE),
    ];
    expect(fanOf(cards).map((card) => card.id)).toEqual(["b", "c", "d", "e", "f"]);
  });

  it("falls back on the bare ones once there are no posters left", () => {
    const cards = [work("a", []), work("b", APICTURE), work("c", [])];
    // The one with a poster first, then the bare ones in the order they
    // arrived: the fan is filled as far as it can be, and what cannot be
    // drawn is drawn as the letter it begins with.
    expect(fanOf(cards).map((card) => card.id)).toEqual(["b", "a", "c"]);
  });

  it("hands back what little there is rather than padding it", () => {
    expect(fanOf([]).map((card) => card.id)).toEqual([]);
    expect(fanOf([work("a", APICTURE)]).map((card) => card.id)).toEqual(["a"]);
  });
});
