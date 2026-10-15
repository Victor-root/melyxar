import { describe, expect, it } from "vitest";
import { cuesOf, secondsOf } from "./vtt";

describe("secondsOf", () => {
  it("reads times with and without hours", () => {
    expect(secondsOf("00:01.500")).toBe(1.5);
    expect(secondsOf("01:02:03.004")).toBeCloseTo(3723.004);
    expect(secondsOf("12:34,567")).toBeCloseTo(754.567);
  });

  it("refuses what is not a time", () => {
    expect(secondsOf("1.5")).toBeNull();
    expect(secondsOf("00:1.500")).toBeNull();
  });
});

describe("cuesOf", () => {
  it("reads every cue with its times and lines, settings left aside", () => {
    const file = [
      "﻿WEBVTT",
      "",
      "NOTE written by the server",
      "",
      "1",
      "00:00:01.000 --> 00:00:03.500 line:85%",
      "First line",
      "<i>second line</i>",
      "",
      "00:04.000 --> 00:05.000",
      "Without a number",
      "",
    ].join("\r\n");
    expect(cuesOf(file)).toEqual([
      { start: 1, end: 3.5, text: "First line\n<i>second line</i>" },
      { start: 4, end: 5, text: "Without a number" },
    ]);
  });

  it("skips a block whose times cannot be read", () => {
    expect(cuesOf("WEBVTT\n\nnonsense --> 00:01.000\nNo\n\n00:02.000 --> 00:03.000\nYes")).toEqual([
      { start: 2, end: 3, text: "Yes" },
    ]);
  });
});
