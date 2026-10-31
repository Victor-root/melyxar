import { describe, expect, it } from "vitest";
import { codeRead } from "./app-scanner";

describe("the code a QR code at a door holds", () => {
  it("is the one its address carries, whatever server it names", () => {
    expect(codeRead("http://192.0.2.10:2100/link?code=482917")).toBe("482917");
    expect(codeRead("https://media.example/link?code=004213")).toBe("004213");
  });

  it("is six digits read as they are", () => {
    expect(codeRead(" 482917 ")).toBe("482917");
  });

  it("is nothing for anything else", () => {
    expect(codeRead("https://media.example/link?code=48291")).toBeNull();
    expect(codeRead("hello")).toBeNull();
  });
});
