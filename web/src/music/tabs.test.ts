import { describe, expect, it } from "vitest";
import { MUSIC_TABS, openTab, shownTabs, swipeOf, tabBeside } from "./tabs";

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

describe("a swipe between the tabs of a phone", () => {
  it("goes to the next tab when the finger goes left, and back when it goes right", () => {
    expect(swipeOf(-80, 10)).toBe(1);
    expect(swipeOf(90, -20)).toBe(-1);
  });

  it("is no swipe when too short or more up and down than sideways", () => {
    expect(swipeOf(-40, 0)).toBe(0);
    expect(swipeOf(-80, 60)).toBe(0);
  });

  it("finds the tab beside, and nothing past either end", () => {
    const shown = shownTabs(["genres"]);
    expect(tabBeside(shown, "for_you", 1)).toBe("albums");
    expect(tabBeside(shown, "albums", -1)).toBe("for_you");
    expect(tabBeside(shown, "for_you", -1)).toBeNull();
    expect(tabBeside(shown, "favourites", 1)).toBeNull();
  });
});
