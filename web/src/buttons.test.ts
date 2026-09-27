import { describe, expect, it } from "vitest";
import { buttonOrder, EVERY_HEADER_BUTTON, knownButtons } from "./buttons";

describe("knownButtons", () => {
  it("keeps each known button once and drops the rest", () => {
    expect(knownButtons(["favourites", "cast", "favourites", ""])).toEqual(["favourites"]);
  });
});

describe("buttonOrder", () => {
  it("holds every button once, those never placed at the end", () => {
    expect(buttonOrder(["watch_later", "watch_later"])).toEqual([
      "watch_later",
      "search",
      "notifications",
      "favourites",
    ]);
  });

  it("is the usual order when nothing was chosen", () => {
    expect(buttonOrder([])).toEqual(EVERY_HEADER_BUTTON);
  });
});
