import { describe, expect, it } from "vitest";
import { kindsOf, pieOf } from "./pie";

describe("the colours of the count on the bell", () => {
  it("has nothing for nothing", () => {
    expect(pieOf([])).toBeUndefined();
  });

  it("is a single colour when all that waits is of one kind, however many", () => {
    expect(pieOf(["trouble", "trouble", "trouble"])).toBe("var(--trouble)");
  });

  it("shares the background equally between the kinds, whatever their numbers", () => {
    expect(pieOf(["trouble", "ok", "ok", "ok"])).toBe(
      "conic-gradient(var(--ok) 0% 50%, var(--trouble) 50% 100%)",
    );
    const three = pieOf(["news", "attention", "ok"]);
    expect(three).toContain("var(--ok) 0% 33.33333333333333%");
    expect(three).toContain("var(--attention) 66.66666666666666% 100%");
  });

  it("lays the kinds in the order of their gravity", () => {
    expect(kindsOf(["trouble", "news", "ok"])).toEqual(["ok", "news", "trouble"]);
  });
});
