import { describe, expect, it } from "vitest";
import { inTurn } from "./home";

describe("the newest albums of several libraries", () => {
  it("takes one from each in turn, up to the room there is", () => {
    expect(inTurn([["a1", "a2", "a3"], ["b1"]], 10)).toEqual(["a1", "b1", "a2", "a3"]);
    expect(inTurn([["a1", "a2"], ["b1", "b2"]], 3)).toEqual(["a1", "b1", "a2"]);
    expect(inTurn([], 5)).toEqual([]);
  });
});
