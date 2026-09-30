import { describe, expect, it } from "vitest";
import { uploadAddress } from "./uploading";

describe("where a file is sent", () => {
  it("names the folder under a root, and leaves it out for the root itself", () => {
    expect(uploadAddress("lib", { root: "r1", folder: " Amber Field/Early Days " }, "a b.mp3")).toBe(
      "/api/v1/libraries/lib/upload?name=a+b.mp3&root=r1&folder=Amber+Field%2FEarly+Days",
    );
    expect(uploadAddress("lib", { root: "r1", folder: "" }, "a.mkv")).toBe(
      "/api/v1/libraries/lib/upload?name=a.mkv&root=r1",
    );
  });

  it("names the album whose folder it goes into", () => {
    expect(uploadAddress("lib", { album: "al1" }, "07 - Tides.flac")).toBe(
      "/api/v1/libraries/lib/upload?name=07+-+Tides.flac&album=al1",
    );
  });
});
