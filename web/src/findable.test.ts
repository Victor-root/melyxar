import { describe, expect, it } from "vitest";
import { addressOf, FINDABLE, find, folded } from "./findable";

/** Every page of settings and of the administration, as written. */
const PAGES = import.meta.glob("./pages/{admin,settings}/*.tsx", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

/** The keys a page names its cards and settings by, read the way the pages
 *  write them. */
function namedIn(file: string): string[] {
  const text = PAGES[`./${file}`];
  if (text === undefined) {
    throw new Error(`no page ${file}`);
  }
  const keys: string[] = [];
  for (const tag of text.matchAll(/<(?:Setting|Panel)\b([\s\S]*?)>/g)) {
    const named = /(?:label|title)=\{t\("([a-z0-9_.]+)"/.exec(tag[1]);
    if (named) {
      keys.push(named[1]);
    }
  }
  return keys;
}

describe("FINDABLE", () => {
  it("holds every setting the pages name, and nothing they no longer do", () => {
    for (const section of FINDABLE) {
      const named = new Set(section.files.flatMap(namedIn));
      expect(new Set(section.keys), section.files.join(", ")).toEqual(named);
    }
  });
});

describe("find", () => {
  const words: Record<string, string> = {
    "admin.transcoding": "Transcodage",
    "admin.limit_sessions": "Limiter les transcodages simultanés",
    "admin.codecs": "Formats de sortie",
  };
  const t = (key: string) => words[key] ?? key;

  it("finds by every word, whatever the case and the accents", () => {
    const found = find("SIMULTANES transcod", "admin", t, 10);
    expect(found.map((one) => one.key)).toEqual(["admin.limit_sessions"]);
    expect(found[0].section).toBe("Transcodage");
  });

  it("finds a section by its own name, before what it holds", () => {
    const found = find("transcod", "admin", t, 10);
    expect(found[0]).toMatchObject({ key: null, path: "transcoding" });
  });

  it("finds nothing for nothing", () => {
    expect(find("  ", "admin", t, 10)).toEqual([]);
  });
});

describe("addressOf", () => {
  it("leads to the section, told which setting to show", () => {
    expect(
      addressOf({ area: "admin", path: "transcoding", said: "", section: "", key: "admin.codecs" }),
    ).toBe("/admin/transcoding?find=admin.codecs");
    expect(addressOf({ area: "settings", path: "", said: "", section: "", key: null })).toBe(
      "/settings",
    );
  });
});

describe("folded", () => {
  it("drops case, accents and marks", () => {
    expect(folded("Arrière-plan  ÉCRAN")).toBe("arriere plan ecran");
  });
});
