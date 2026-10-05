import { beforeEach, describe, expect, it, vi } from "vitest";
import { nameTheTab, showListening, showPage, showPlaying } from "./tab";

describe("the name of the tab", () => {
  beforeEach(() => {
    vi.stubGlobal("document", { title: "Melyxar" });
    showPlaying(null);
    showListening(null);
    showPage(null);
    nameTheTab("Home server");
  });

  it("is the server's name on its own", () => {
    expect(document.title).toBe("Home server");
  });

  it("follows the server's name with the page being read", () => {
    showPage("Films");
    expect(document.title).toBe("Home server · Films");
    showPage(null);
    expect(document.title).toBe("Home server");
  });

  it("gives way to what is playing or heard, and comes back after", () => {
    showPage("Films");
    showListening("A song");
    expect(document.title).toBe("A song");
    showPlaying("A film");
    expect(document.title).toBe("A film");
    showPlaying(null);
    showListening(null);
    expect(document.title).toBe("Home server · Films");
  });

  it("falls back to the title it was given before the server is named", () => {
    nameTheTab(null);
    showPage("Series");
    expect(document.title).toBe("Melyxar · Series");
  });
});
