import { describe, expect, it } from "vitest";
import { hasTheWholeScreen, leavesRoom } from "./edge-room";

describe("leavesRoom", () => {
  it("finds no room where the browser reads none", () => {
    expect(leavesRoom(["0px", "0px", "0px", "0px"])).toBe(false);
  });

  it("finds room on any one side, the status bar or the navigation bar alone", () => {
    expect(leavesRoom(["30px", "0px", "0px", "0px"])).toBe(true);
    expect(leavesRoom(["0px", "48px", "0px", "0px"])).toBe(true);
    expect(leavesRoom(["0px", "0px", "0px", "12px"])).toBe(true);
  });

  it("treats a value it cannot read as no room", () => {
    expect(leavesRoom(["", "", "", ""])).toBe(false);
  });
});

describe("hasTheWholeScreen", () => {
  const BROWSER = "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 Chrome/150.0 Mobile Safari/537.36";

  it("is the whole screen in an installed app", () => {
    expect(hasTheWholeScreen(true, BROWSER)).toBe(true);
  });

  it("is the whole screen in Melyxar's own application", () => {
    expect(hasTheWholeScreen(false, `${BROWSER} MelyxarApp/0.1.0`)).toBe(true);
  });

  it("is not in a browser tab", () => {
    expect(hasTheWholeScreen(false, BROWSER)).toBe(false);
  });
});
