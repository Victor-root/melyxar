import { describe, expect, it } from "vitest";
import type { EditedTags } from "./api";
import { albumFieldsOf, namesField, namesIn, numberOf, textOf, withAlbum } from "./tag-form";

const tides: EditedTags = {
  title: "Tides",
  artists: ["Amber Field"],
  album: "Northern Lights",
  album_artists: ["Amber Field"],
  track: 2,
  disc: 1,
  year: 2019,
  genres: ["Folk", "Pop"],
  compilation: false,
};

describe("the fields of the tag manager", () => {
  it("reads names, numbers and text back from what was typed", () => {
    expect(namesIn(" Amber Field ;The Lanterns;; ")).toEqual(["Amber Field", "The Lanterns"]);
    expect(namesField(["Amber Field", "The Lanterns"])).toBe("Amber Field; The Lanterns");
    expect(numberOf(" 7 ")).toBe(7);
    expect(numberOf("")).toBeNull();
    expect(numberOf("2.5")).toBeNull();
    expect(numberOf("0")).toBeNull();
    expect(textOf("  ")).toBeNull();
    expect(textOf(" Tides ")).toBe("Tides");
  });

  it("lays the album's own fields over every song, keeping what is each song's", () => {
    const album = { ...albumFieldsOf(tides), album: "Northern Lights (Deluxe)", year: "2020", genres: "Folk" };
    const laid = withAlbum({ ...tides, title: "Low Tide", track: 5 }, album);
    expect(laid.album).toBe("Northern Lights (Deluxe)");
    expect(laid.year).toBe(2020);
    expect(laid.genres).toEqual(["Folk"]);
    expect(laid.title).toBe("Low Tide");
    expect(laid.track).toBe(5);
    expect(albumFieldsOf(undefined).album).toBe("");
  });
});
