import { describe, expect, it } from "vitest";
import { buttonOrder, EVERY_HEADER_BUTTON, knownButtons, offeredButtons } from "./buttons";

describe("knownButtons", () => {
  it("keeps each known button once and drops the rest", () => {
    expect(knownButtons(["favourites", "radio", "favourites", ""])).toEqual(["favourites"]);
  });
});

describe("buttonOrder", () => {
  it("holds every button once, those never placed at the end", () => {
    expect(buttonOrder(["watch_later", "watch_later"])).toEqual([
      "watch_later",
      "search",
      "favourites",
      "collections",
      "playlists",
      "requests",
      "notifications",
      "scan",
      "administration",
      "cast",
      "settings",
    ]);
  });

  it("is the usual order when nothing was chosen", () => {
    expect(buttonOrder([])).toEqual(EVERY_HEADER_BUTTON);
  });
});

describe("offeredButtons", () => {
  it("keeps requests for an account that may ask, and the administration for an administrator", () => {
    const all = (administrator: boolean, mayRequest: boolean) =>
      offeredButtons(EVERY_HEADER_BUTTON, { administrator, mayRequest });
    expect(all(false, false)).not.toContain("requests");
    expect(all(false, false)).not.toContain("administration");
    expect(all(false, true)).toContain("requests");
    expect(all(false, true)).not.toContain("scan");
    expect(all(true, false)).toContain("scan");
    expect(all(true, false)).not.toContain("requests");
    expect(all(true, true)).toEqual(EVERY_HEADER_BUTTON);
  });
});
