import { describe, expect, it } from "vitest";
import type { Library } from "../api";
import type { Found } from "./api";
import { foundAny, musicScopeOf, quickLinesOf } from "./search";
import { aSong } from "./testing";

const films = { id: "f", kind: "movies" } as Library;
const songs = { id: "m", kind: "music" } as Library;

describe("where a search looks for music", () => {
  it("looks everywhere only when the server holds music", () => {
    expect(musicScopeOf("", [films, songs])).toEqual({ looks: true, only: false });
    expect(musicScopeOf("", [films])).toEqual({ looks: false, only: false });
  });

  it("looks only in music when narrowed to it, and nowhere when narrowed elsewhere", () => {
    expect(musicScopeOf("kind:music", [songs])).toEqual({ looks: true, only: true });
    expect(musicScopeOf("library:m", [films, songs])).toEqual({ looks: true, library: "m", only: true });
    expect(musicScopeOf("library:f", [films, songs])).toEqual({ looks: false, only: false });
    expect(musicScopeOf("kind:movies", [films, songs])).toEqual({ looks: false, only: false });
  });
});

describe("what a search found in the music", () => {
  const found: Found = {
    artists: [{ id: "a", library: "m", name: "Amber Field", initial: "a", albums: 1, songs: 2, color: null, picture: [] }],
    albums: [{ id: "b", library: "m", title: "Road", artists: [{ id: "a", name: "Amber Field" }], compilation: false, year: 2015, songs: 1, color: "#123", initial: "r", cover: [] }],
    songs: [
      aSong({ id: "c", title: "Long Road", artists: [{ id: "a", name: "Amber Field" }], album: { id: "b", name: "Road" } }),
      aSong({ id: "d", title: "Loose" }),
    ],
  };

  it("lists artists, then albums, then songs leading to their album, within the room", () => {
    expect(quickLinesOf(found, 10).map((line) => [line.to, line.shape])).toEqual([
      ["/music/artist/a", "round"],
      ["/music/album/b", "square"],
      ["/music/album/b", "square"],
    ]);
    expect(quickLinesOf(found, 2)).toHaveLength(2);
    expect(quickLinesOf(null, 5)).toEqual([]);
  });

  it("says whether anything was found at all", () => {
    expect(foundAny(found)).toBe(true);
    expect(foundAny({ albums: [], artists: [], songs: [] })).toBe(false);
    expect(foundAny(null)).toBe(false);
  });
});
