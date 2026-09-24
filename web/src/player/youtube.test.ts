import { describe, expect, it } from "vitest";
import { youTubeKey } from "./youtube";

describe("youTubeKey", () => {
  it("reads the video out of the links the server writes, and the short ones", () => {
    expect(youTubeKey("https://www.youtube.com/watch?v=abc123XYZ_-")).toBe("abc123XYZ_-");
    expect(youTubeKey("https://m.youtube.com/watch?v=abc&t=10")).toBe("abc");
    expect(youTubeKey("https://youtu.be/abc123")).toBe("abc123");
  });

  it("names nothing for another site, another page, or no link at all", () => {
    expect(youTubeKey("https://vimeo.com/12345")).toBeNull();
    expect(youTubeKey("https://www.youtube.com/channel/abc")).toBeNull();
    expect(youTubeKey("https://www.youtube.com/watch")).toBeNull();
    expect(youTubeKey("not a link")).toBeNull();
  });
});
