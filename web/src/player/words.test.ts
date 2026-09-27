/*
 * The lines drawn for the words on screen.
 */

import { describe, expect, it } from "vitest";
import { linesOf } from "./words";

describe("the lines drawn for the words on screen", () => {
  it("are one entry per line, cue after cue", () => {
    expect(linesOf(["First line\nSecond line", "Another cue"])).toEqual([
      "First line",
      "Second line",
      "Another cue",
    ]);
  });

  it("carry none of the tags a subtitle file can hold", () => {
    expect(linesOf(["<i>An aside</i>", '<c.yellow>Coloured</c> <b>bold</b>'])).toEqual([
      "An aside",
      "Coloured bold",
    ]);
  });

  it("are none when nothing is said", () => {
    expect(linesOf([])).toEqual([]);
  });
});
