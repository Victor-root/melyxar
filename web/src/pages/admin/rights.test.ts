import { describe, expect, it } from "vitest";
import type { Library } from "../../api";
import { AN_ORDINARY_ACCOUNT, granting, reachOf, settled } from "./rights";

const t = (key: string) => key;

function library(id: string, name: string): Library {
  return {
    id,
    name,
    kind: "movies",
    works: 0,
    version: 0,
    key_frames_during_scan: false,
    thumbnails_during_scan: false,
    watch_in_real_time: false,
    metadata_language: "fr",
    roots: [],
    set_aside: 0,
  };
}

describe("settled", () => {
  it("gives an administrator every right whatever was asked", () => {
    expect(
      settled({ ...AN_ORDINARY_ACCOUNT, is_administrator: true, sees_every_library: false, most_streams: 1 }),
    ).toEqual({
      is_administrator: true,
      sees_every_library: true,
      libraries: [],
      may_delete: true,
      may_delete_from_disk: true,
      most_streams: null,
    });
  });

  it("never lets erasing from the disk go without the right to delete", () => {
    expect(settled({ ...AN_ORDINARY_ACCOUNT, may_delete_from_disk: true }).may_delete_from_disk).toBe(false);
    expect(
      settled({ ...AN_ORDINARY_ACCOUNT, may_delete: true, may_delete_from_disk: true }).may_delete_from_disk,
    ).toBe(true);
  });

  it("keeps no list for an account that sees every library", () => {
    expect(settled({ ...AN_ORDINARY_ACCOUNT, libraries: ["films"] }).libraries).toEqual([]);
  });
});

describe("granting", () => {
  const limited = { ...AN_ORDINARY_ACCOUNT, sees_every_library: false };

  it("adds a library once and takes it away again", () => {
    const once = granting(granting(limited, "films", true), "films", true);
    expect(once.libraries).toEqual(["films"]);
    expect(granting(once, "films", false).libraries).toEqual([]);
  });
});

describe("reachOf", () => {
  const libraries = [library("films", "Films"), library("series", "Series")];

  it("names what an account reaches", () => {
    expect(reachOf(AN_ORDINARY_ACCOUNT, libraries, t)).toBe("users.every_library");
    expect(
      reachOf({ ...AN_ORDINARY_ACCOUNT, sees_every_library: false, libraries: ["series"] }, libraries, t),
    ).toBe("Series");
    expect(reachOf({ ...AN_ORDINARY_ACCOUNT, sees_every_library: false }, libraries, t)).toBe(
      "users.no_library_short",
    );
  });
});
