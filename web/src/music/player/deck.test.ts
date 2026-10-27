import { describe, expect, it } from "vitest";

import { aboutASongLetGo } from "./deck";

describe("aboutASongLetGo", () => {
  it("listens to everything while the deck keeps its song", () => {
    expect(aboutASongLetGo(3, null)).toBe(false);
  });

  it("leaves out the words about the song the deck was given another for", () => {
    expect(aboutASongLetGo(3, 3)).toBe(true);
    expect(aboutASongLetGo(2, 3)).toBe(true);
  });

  it("listens again from the first word about the new song", () => {
    expect(aboutASongLetGo(4, 3)).toBe(false);
  });
});
