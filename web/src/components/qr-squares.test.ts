import { describe, expect, it } from "vitest";
import { pathOf } from "./qr-squares";

describe("a QR code drawn as one shape", () => {
  it("draws a run of dark squares in a row as one piece, inside its margin", () => {
    expect(pathOf(["0110", "1001"])).toBe("M5 4h2v1h-2zM4 5h1v1h-1zM7 5h1v1h-1z");
  });

  it("draws nothing for a light row", () => {
    expect(pathOf(["000"])).toBe("");
  });
});
