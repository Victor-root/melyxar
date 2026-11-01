import { describe, expect, it } from "vitest";
import type { PlaybackTrack } from "../api";
import { trackName } from "./describe";

const said = (key: string) => key;

function track(more: Partial<PlaybackTrack>): PlaybackTrack {
  return { id: "1", language: "fre", title: null, codec: "eac3", is_default: false, channels: null, burns_in: null, url: null, ...more };
}

describe("trackName", () => {
  it("names a soundtrack by its language, its sound and its title", () => {
    expect(trackName(track({ channels: 6 }), said, "fr")).toBe("Français · Dolby Digital+ 5.1");
    expect(trackName(track({ channels: 2, codec: "aac", title: "Commentaire" }), said, "fr")).toBe("Français · AAC · Commentaire");
  });

  it("says nothing of the sound of a subtitle", () => {
    expect(trackName(track({ codec: "subrip" }), said, "fr")).toBe("Français");
  });
});
