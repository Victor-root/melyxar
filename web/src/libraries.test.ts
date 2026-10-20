/*
 * The kinds of library held, what a category is called, and how cards are
 * laid out.
 */

import { describe, expect, it } from "vitest";
import type { Library, LibraryKind } from "./api";
import { cardShapeOf, holdsFiles, kindsHeld, KINDS, nameOfKind } from "./libraries";

/** A library with only what the order reads of it. */
function library(kind: LibraryKind): Library {
  return { kind } as Library;
}

const EVERY: LibraryKind[] = ["movies", "series", "anime", "shows", "music"];

describe("the kinds held", () => {
  it("are the kinds this account holds a library of, in the order given", () => {
    const held = [library("music"), library("movies")];
    expect(kindsHeld(EVERY, held)).toEqual(["movies", "music"]);
  });
});

describe("what a library is made of", () => {
  it("is files found in folders for every kind but cameras", () => {
    expect(KINDS.filter((kind) => !holdsFiles(kind))).toEqual(["cameras"]);
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
  });

  it("is the name of the kind when it gathers several", () => {
    const held = [named("movies", "Disk one"), named("movies", "Disk two")];
    expect(nameOfKind("movies", held, t)).toBe("kind.movies");
  });
});
