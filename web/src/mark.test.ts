import { describe, expect, it } from "vitest";
import { vividOf } from "./mark";

describe("vividOf", () => {
  it("gives the usual accent the red the logo was drawn in", () => {
    expect(vividOf("#c81e1e")).toBe("#e50000");
  });

  it("keeps the hue of any accent and makes it as vivid", () => {
    expect(vividOf("#1c7ed6")).toBe("#0079e6");
    expect(vividOf("#2f9e44")).toBe("#1fc73f");
  });

  it("leaves a grey grey", () => {
    expect(vividOf("#808080")).toBe("#737373");
  });
});
