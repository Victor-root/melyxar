import { describe, expect, it } from "vitest";
import type { Album } from "./api";
import { coversOf } from "./band";

const album = (id: string, covered: boolean) =>
  ({ id, cover: covered ? [{ url: id }] : [] }) as unknown as Album;

describe("the covers the tile of music fans out", () => {
  it("puts the albums with a cover first, the newest first, five at most", () => {
    const albums = [album("a", false), album("b", true), album("c", true), album("d", false), album("e", true), album("f", true), album("g", true)];
    expect(coversOf(albums).map((one) => one.id)).toEqual(["b", "c", "e", "f", "g"]);
    expect(coversOf([album("a", false)]).map((one) => one.id)).toEqual(["a"]);
  });
});
