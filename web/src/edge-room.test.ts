import { describe, expect, it } from "vitest";
import { leavesRoom } from "./edge-room";

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
