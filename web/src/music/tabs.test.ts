import { describe, expect, it } from "vitest";
import { MUSIC_TABS, openTab, shownTabs } from "./tabs";

describe("the tabs of a library of music", () => {
  it("keeps their order and leaves out what is hidden", () => {
    expect(shownTabs(["songs", "genres"])).toEqual([
      "for_you",
      "albums",
      "album_artists",
      "artists",
      "playlists",
      "favourites",
    ]);
  });

  it("never hides every one", () => {
    expect(shownTabs([...MUSIC_TABS])).toEqual([...MUSIC_TABS]);
  });

  it("opens on the tab asked for when it is shown, and on the first otherwise", () => {
    const shown = shownTabs(["for_you"]);
    expect(openTab("songs", shown)).toBe("songs");
    expect(openTab("for_you", shown)).toBe("albums");
    expect(openTab(null, shown)).toBe("albums");
  });
});
