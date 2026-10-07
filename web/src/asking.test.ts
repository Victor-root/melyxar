import { describe, expect, it } from "vitest";
import { ApiError } from "./api";
import { heldBackFor } from "./asking";

describe("heldBackFor", () => {
  it("reads how long the brake on wrong passwords asks to wait", () => {
    expect(heldBackFor(new ApiError("too_many_attempts", 429, undefined, { seconds: 240 }))).toBe(240);
  });

  it("falls back on a minute when the wait was not said", () => {
    expect(heldBackFor(new ApiError("too_many_attempts", 429))).toBe(60);
  });

  it("says nothing for any other refusal", () => {
    expect(heldBackFor(new ApiError("unauthenticated", 401))).toBeNull();
    expect(heldBackFor(new Error("offline"))).toBeNull();
  });
});
