import { describe, expect, it } from "vitest";
import type { PlaybackThumbnails } from "../api";
import { cutOut, turnedBox } from "./thumbnail";

const sheets: PlaybackThumbnails = {
  url: "/sheets",
  every_seconds: 10,
  width: 320,
  height: 180,
  columns: 10,
  rows: 10,
  counted: 250,
};

describe("cutOut", () => {
  it("stretches the sheet so one thumbnail fills the box", () => {
    expect(cutOut(sheets, 0)).toEqual({
      backgroundImage: "url(/sheets/0.jpg)",
      backgroundSize: "1000% 1000%",
      backgroundPosition: "0% 0%",
    });
  });

  it("slides to the column and row of the moment, on its own sheet", () => {
    // The 123rd thumbnail: second sheet, third row, fourth column.
    const cut = cutOut(sheets, 1234);
    expect(cut?.backgroundImage).toBe("url(/sheets/1.jpg)");
    expect(cut?.backgroundPosition).toBe(`${(3 / 9) * 100}% ${(2 / 9) * 100}%`);
  });

  it("never moves a sheet of one column, and stops at the last thumbnail", () => {
    const narrow = { ...sheets, columns: 1, rows: 4, counted: 4 };
    expect(cutOut(narrow, 9999)?.backgroundPosition).toBe("0% 100%");
  });
});

describe("turnedBox", () => {
  it("swaps the two sides for a quarter turn either way", () => {
    expect(turnedBox(240, 135, 90)).toEqual({ across: 135, down: 240 });
    expect(turnedBox(240, 135, 270)).toEqual({ across: 135, down: 240 });
  });

  it("keeps them for no turn and for a half turn", () => {
    expect(turnedBox(240, 135, 0)).toEqual({ across: 240, down: 135 });
    expect(turnedBox(240, 135, 180)).toEqual({ across: 240, down: 135 });
  });
});
