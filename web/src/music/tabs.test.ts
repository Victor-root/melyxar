import { describe, expect, it } from "vitest";
import { MUSIC_TABS, openTab, shownTabs, tabBeside } from "./tabs";

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

describe("the tab beside another", () => {
  it("is the next or the one before, and nothing past either end", () => {
    const shown = shownTabs(["genres"]);
    expect(tabBeside(shown, "for_you", 1)).toBe("albums");
    expect(tabBeside(shown, "albums", -1)).toBe("for_you");
    expect(tabBeside(shown, "for_you", -1)).toBeNull();
    expect(tabBeside(shown, "favourites", 1)).toBeNull();
  });
});
