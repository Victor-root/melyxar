import { describe, expect, it } from "vitest";
import { lightFilter, lightName } from "./picture-light";

describe("lightFilter", () => {
  it("leaves the picture alone at a hundred percent", () => {
    expect(lightFilter(1, 1)).toBeUndefined();
  });

  it("lights then contrasts, as asked", () => {
    expect(lightFilter(1.2, 1)).toBe("brightness(1.2)");
    expect(lightFilter(1, 0.9)).toBe("contrast(0.9)");
    expect(lightFilter(1.3, 1.1)).toBe("brightness(1.3) contrast(1.1)");
  });
});

describe("lightName", () => {
  it("writes a share in the reader's language", () => {
    expect(lightName(1.2, "en")).toBe("120%");
    expect(lightName(0.8, "fr").replace(/\s/u, " ")).toBe("80 %");
  });
});
