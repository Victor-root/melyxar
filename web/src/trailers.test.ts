import { describe, expect, it } from "vitest";
import { embedOf } from "./trailers";

describe("embedOf", () => {
  it("gives YouTube's own player for the links the server writes, and the short ones", () => {
    expect(embedOf("https://www.youtube.com/watch?v=abc123XYZ_-")).toBe(
      "https://www.youtube-nocookie.com/embed/abc123XYZ_-?autoplay=1&rel=0",
    );
    expect(embedOf("https://youtu.be/abc123")).toBe(
      "https://www.youtube-nocookie.com/embed/abc123?autoplay=1&rel=0",
    );
  });

  it("gives Vimeo's own player for a Vimeo video", () => {
    expect(embedOf("https://vimeo.com/12345")).toBe("https://player.vimeo.com/video/12345?autoplay=1");
  });

  it("gives nothing for another page, another site, or no link at all", () => {
    expect(embedOf("https://www.youtube.com/channel/abc")).toBeNull();
    expect(embedOf("https://www.youtube.com/watch")).toBeNull();
    expect(embedOf("https://vimeo.com/channels/staff")).toBeNull();
    expect(embedOf("https://example.com/trailer")).toBeNull();
    expect(embedOf("not a link")).toBeNull();
  });
});
