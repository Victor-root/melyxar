import { describe, expect, it } from "vitest";
import { FACE_PICTURES, facePictureOf } from "./faces";

/** What these names wear, which the television's FacesTest checks too. */
const PICKED = ["comet", "orbits", "comet", "comet", "rings", "orbits", "crescent"];

describe("facePictureOf", () => {
  it("gives the same name the same picture every time", () => {
    expect(facePictureOf("Alice")).toBe(facePictureOf("Alice"));
  });

  it("spreads a crowd of names over every picture", () => {
    const names = Array.from({ length: 400 }, (_, at) => `account ${at}`);
    expect(new Set(names.map(facePictureOf)).size).toBe(FACE_PICTURES.length);
  });

  it("picks what the television picks for the same names", () => {
    expect(["Victor", "Élodie", "Zoë", "Marie-Hélène", "Bob", "", "🎬 Cinéma"].map(facePictureOf)).toEqual(
      PICKED,
    );
  });
});
