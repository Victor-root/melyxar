import { describe, expect, it } from "vitest";

import { pressedFrom, pressedOnTheCard, songOfAddress } from "./journal";

describe("songOfAddress", () => {
  it("reads the song out of the address of its sound", () => {
    expect(songOfAddress("/api/v1/music/songs/01a0f0c6-c220/sound?plays=mp3&start=12.000")).toBe("01a0f0c6-c220");
    expect(songOfAddress("https://server/api/v1/music/songs/abc/sound")).toBe("abc");
  });

  it("gives nothing for anything else", () => {
    expect(songOfAddress(null)).toBeNull();
    expect(songOfAddress("")).toBeNull();
    expect(songOfAddress("/api/v1/stream/abc/init.mp4")).toBeNull();
  });
});

describe("pressedFrom", () => {
  it("is the card only while a press of the card is done", () => {
    let during: string | null = null;
    pressedOnTheCard(() => {
      during = pressedFrom();
    });
    expect(during).toBe("card");
    expect(pressedFrom()).toBe("interface");
  });

  it("is the interface again after a press of the card that failed", () => {
    expect(() =>
      pressedOnTheCard(() => {
        throw new Error("refused");
      }),
    ).toThrow();
    expect(pressedFrom()).toBe("interface");
  });
});
