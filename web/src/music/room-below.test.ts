import { describe, expect, it } from "vitest";
import { roomBelow } from "./room-below";

describe("roomBelow", () => {
  it("takes what is left under the top once the foot is kept", () => {
    expect(roomBelow(1000, 400, 60, 280)).toBe(540);
  });

  it("never goes under the least, on a screen too short", () => {
    expect(roomBelow(600, 400, 60, 280)).toBe(280);
  });
});
