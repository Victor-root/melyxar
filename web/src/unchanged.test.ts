import { describe, expect, it } from "vitest";
import { keepTheUnchanged } from "./unchanged";

describe("keeping what did not change", () => {
  it("hands back the old object for an item that says the same", () => {
    const before = [{ id: "a", title: "A" }];
    const after = [{ id: "a", title: "A" }];

    expect(keepTheUnchanged(before, after)[0]).toBe(before[0]);
  });

  it("takes the new object for an item that changed", () => {
    const before = [{ id: "a", title: "A" }];
    const after = [{ id: "a", title: "B" }];

    expect(keepTheUnchanged(before, after)[0]).toBe(after[0]);
  });

  it("follows the order and the content of the new list", () => {
    const before = [{ id: "a" }, { id: "b" }];
    const after = [{ id: "b" }, { id: "c" }];
    const kept = keepTheUnchanged(before, after);

    expect(kept.map((item) => item.id)).toEqual(["b", "c"]);
    expect(kept[0]).toBe(before[1]);
    expect(kept[1]).toBe(after[1]);
  });

  it("copes with an empty old list", () => {
    const after = [{ id: "a" }];

    expect(keepTheUnchanged([], after)[0]).toBe(after[0]);
  });
});
