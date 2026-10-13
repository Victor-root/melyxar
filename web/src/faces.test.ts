import { describe, expect, it } from "vitest";
import { FACE_DRAWINGS, FACE_TURNS, faceArtOf } from "./faces";

describe("faceArtOf", () => {
  it("gives the same name the same painting every time", () => {
    expect(faceArtOf("Alice")).toEqual(faceArtOf("Alice"));
  });

  it("only ever picks a drawing and a turn that exist", () => {
    for (const name of ["", "a", "Bruno", "Élodie", "名前", "tester 2"]) {
      const art = faceArtOf(name);
      expect(FACE_DRAWINGS).toContain(art.drawing);
      expect(FACE_TURNS).toContain(art.turn);
    }
  });

  it("spreads a crowd of names over every drawing and every turn", () => {
    const names = Array.from({ length: 400 }, (_, at) => `account ${at}`);
    const drawings = new Set(names.map((name) => faceArtOf(name).drawing));
    const turns = new Set(names.map((name) => faceArtOf(name).turn));
    expect(drawings.size).toBe(FACE_DRAWINGS.length);
    expect(turns.size).toBe(FACE_TURNS.length);
  });

  it("tells names apart that differ only in case", () => {
    const names = ["alice", "Alice", "ALICE", "aLICE"];
    const paintings = new Set(names.map((name) => JSON.stringify(faceArtOf(name))));
    expect(paintings.size).toBeGreaterThan(1);
  });
});
