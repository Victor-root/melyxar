import { describe, expect, it } from "vitest";
import { pressed } from "./selecting";

const order = ["a", "b", "c", "d", "e"];

describe("pressed", () => {
  it("turns one card on, then off", () => {
    const on = pressed(order, new Set(), "b", null, false);
    expect([...on]).toEqual(["b"]);
    expect([...pressed(order, on, "b", "b", false)]).toEqual([]);
  });

  it("chooses the whole range from the last card pressed, either way", () => {
    expect([...pressed(order, new Set(["b"]), "d", "b", true)].sort()).toEqual(["b", "c", "d"]);
    expect([...pressed(order, new Set(["d"]), "a", "d", true)].sort()).toEqual([
      "a",
      "b",
      "c",
      "d",
    ]);
  });

  it("only adds with a range, keeping what was chosen before", () => {
    const before = new Set(["a", "c"]);
    expect([...pressed(order, before, "e", "d", true)].sort()).toEqual(["a", "c", "d", "e"]);
  });

  it("is a single press when there is nowhere to start a range from", () => {
    expect([...pressed(order, new Set(), "c", null, true)]).toEqual(["c"]);
    expect([...pressed(order, new Set(), "c", "gone", true)]).toEqual(["c"]);
  });
});
