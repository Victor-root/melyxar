import { describe, expect, it } from "vitest";
import { addressAfter, runningOut } from "./access";

describe("addressAfter", () => {
  it("sends a page in the clear to https once the server encrypts", () => {
    expect(addressAfter("self_signed", true, new URL("http://192.168.1.10:2100/admin/security?x=1"))).toBe(
      "https://192.168.1.10:2100/admin/security?x=1",
    );
  });

  it("leaves a page already encrypted where it is", () => {
    expect(addressAfter("provided", true, new URL("https://media.example/admin/security"))).toBeNull();
  });

  it("brings an encrypted page back to the clear behind a proxy", () => {
    expect(addressAfter("proxy", true, new URL("https://192.168.1.10:2100/admin/security"))).toBe(
      "http://192.168.1.10:2100/admin/security",
    );
  });

  it("leaves a page the proxy encrypts alone", () => {
    expect(addressAfter("proxy", false, new URL("https://media.example/admin/security"))).toBeNull();
  });
});

describe("runningOut", () => {
  const now = new Date("2026-09-30T12:00:00Z");

  it("says so within a month of the end, and after it", () => {
    expect(runningOut("2026-10-15T00:00:00Z", now)).toBe(true);
    expect(runningOut("2026-01-01T00:00:00Z", now)).toBe(true);
    expect(runningOut("2027-09-30T00:00:00Z", now)).toBe(false);
  });
});
