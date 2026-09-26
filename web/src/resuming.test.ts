/*
 * The rules of a film left partway, kind of library by kind of library.
 */

import { describe, expect, it } from "vitest";
import { rulesOf, withRulesOf } from "./resuming";

const EVERY_KIND = { min_percent: 5, max_percent: 90, min_seconds: 120 };
const FILMS = { min_percent: 2, max_percent: 95, min_seconds: 600 };

describe("the rules of a kind of library", () => {
  it("are its own once given, and those of every kind until then", () => {
    const byKind = [{ kind: "movies" as const, ...FILMS }];
    expect(rulesOf(byKind, "movies", EVERY_KIND)).toEqual(FILMS);
    expect(rulesOf(byKind, "anime", EVERY_KIND)).toEqual(EVERY_KIND);
  });

  it("replace what the kind had without touching the others", () => {
    const byKind = [
      { kind: "movies" as const, ...FILMS },
      { kind: "anime" as const, ...EVERY_KIND },
    ];
    const changed = withRulesOf(byKind, "movies", { ...FILMS, min_seconds: 60 });
    expect(rulesOf(changed, "movies", EVERY_KIND).min_seconds).toBe(60);
    expect(rulesOf(changed, "anime", EVERY_KIND)).toEqual(EVERY_KIND);
    expect(changed).toHaveLength(2);
  });
});
