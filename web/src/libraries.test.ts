/*
 * The kinds of library a home page offers, and how their cards are laid out.
 */

import { describe, expect, it } from "vitest";
import type { Library, LibraryKind } from "./api";
import {
  cardShapeOf,
  kindsOnTheHomePage,
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
