import { describe, expect, it } from "vitest";
import type { Found } from "./api";
import { stillLiked } from "./for-you";

describe("what is still liked", () => {
  it("leaves out whatever lost its heart since the server said", () => {
    const found = {
      artists: [{ id: "a" }, { id: "b" }],
      albums: [{ id: "c" }],
      songs: [{ id: "d" }, { id: "e" }],
    } as unknown as Found;
    const liked = (id: string) => id !== "b" && id !== "e";
    const kept = stillLiked(found, liked);
    expect(kept.artists.map((one) => one.id)).toEqual(["a"]);
    expect(kept.albums.map((one) => one.id)).toEqual(["c"]);
    expect(kept.songs.map((one) => one.id)).toEqual(["d"]);
  });
});
