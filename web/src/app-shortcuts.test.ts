import { describe, expect, it } from "vitest";
import type { Library } from "./api";
import { shortcutsFor } from "./app-shortcuts";

const t = (key: string) => key;
const library = (id: string, kind: Library["kind"]) => ({ id, name: id, kind }) as Library;

describe("the shortcuts of the application's icon", () => {
  it("offers the music, the summary, the settings and the camera to an administrator", () => {
    const offered = shortcutsFor([library("films", "movies"), library("songs", "music")], true, t);
    expect(offered.map((shortcut) => shortcut.id)).toEqual(["music", "dashboard", "settings", "link"]);
    expect(offered[0].path).toBe("/library/songs");
  });

  it("leaves out what an account does not reach", () => {
    const offered = shortcutsFor([library("films", "movies")], false, t);
    expect(offered.map((shortcut) => shortcut.id)).toEqual(["settings", "link"]);
  });
});
