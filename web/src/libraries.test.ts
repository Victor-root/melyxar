/*
 * The order of the libraries as the settings screen moves it.
 *
 * The kinds on the screen put in a new order must come out in that order,
 * whatever kinds hide between them in the full order. A slip here moves
 * nothing visible, or moves the wrong row, and looks like a drop that did
 * not take.
 */

import { describe, expect, it } from "vitest";
import type { Library, LibraryKind } from "./api";
import {
  cardShapeOf,
  kindsOnTheHomePage,
  reorderedOnTheHomePage,
  nameOfKind,
  newestOfKind,
} from "./libraries";

/** A library with only what the order reads of it. */
function library(kind: LibraryKind): Library {
  return { kind } as Library;
}

const EVERY: LibraryKind[] = ["movies", "series", "anime", "shows", "music"];

describe("the order of the home page", () => {
  it("shows only the kinds this account holds, never music", () => {
    const held = [library("anime"), library("music"), library("movies"), library("movies")];
    expect(kindsOnTheHomePage(EVERY, held)).toEqual(["movies", "anime"]);
  });

  it("puts the kinds on the screen in their new order, over the kinds nobody sees", () => {
    const shown: LibraryKind[] = ["movies", "anime", "shows"];
    expect(reorderedOnTheHomePage(EVERY, shown, ["shows", "movies", "anime"])).toEqual([
      "shows",
      "series",
      "movies",
      "anime",
      "music",
    ]);
  });

  it("leaves the order alone when nothing moved", () => {
    const shown: LibraryKind[] = ["movies", "anime"];
    expect(reorderedOnTheHomePage(EVERY, shown, shown)).toEqual(EVERY);
  });
});

describe("the shape of the cards of a kind", () => {
  it("lays down what people filmed and photographed themselves, and only that", () => {
    expect(cardShapeOf("home_media")).toBe("lying");
    expect(cardShapeOf("movies")).toBe("standing");
    expect(cardShapeOf(undefined)).toBe("standing");
  });
});

describe("the name of a category", () => {
  const t = (key: string, values?: Record<string, string | number>) =>
    values ? `${key}(${values.name})` : key;
  const named = (kind: LibraryKind, name: string) => ({ kind, name }) as Library;

  it("is the name of its library when it holds only one", () => {
    const held = [named("home_media", "Perso"), named("movies", "Disk one")];
    expect(nameOfKind("home_media", held, t)).toBe("Perso");
    expect(newestOfKind("home_media", held, t)).toBe("home.newest.named(Perso)");
  });

  it("is the name of the kind when it gathers several", () => {
    const held = [named("movies", "Disk one"), named("movies", "Disk two")];
    expect(nameOfKind("movies", held, t)).toBe("kind.movies");
    expect(newestOfKind("movies", held, t)).toBe("home.newest.movies");
  });
});
