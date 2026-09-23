import { describe, expect, it } from "vitest";
import { shareOfThePictureKept } from "./banner";

describe("shareOfThePictureKept", () => {
  it("keeps a little over half of a sixteen by nine picture at the usual height", () => {
    expect(shareOfThePictureKept(0.31)).toBe(55);
  });

  it("never keeps more than the whole picture", () => {
    expect(shareOfThePictureKept(0.8)).toBe(100);
  });
});
