/*
 * How the lines of the journal are told apart from one read to the next.
 */

import { describe, expect, it } from "vitest";
import type { JournalLine } from "../api";
import { lineKeys } from "./journal";

function line(at: string, message: string): JournalLine {
  return { at, level: "info", tag: "scan", module: "melyxar_app::scan", message };
}

describe("the keys of the lines of the journal", () => {
  it("stay the same for a line however far up the list it has moved", () => {
    const before = [line("10:00:01", "a"), line("10:00:02", "b"), line("10:00:03", "c")];
    const after = [...before.slice(1), line("10:00:04", "d")];
    expect(lineKeys(after).slice(0, 2)).toEqual(lineKeys(before).slice(1));
  });

  it("tell apart lines written at the same instant", () => {
    const keys = lineKeys([line("10:00:01", "a"), line("10:00:01", "b"), line("10:00:02", "c")]);
    expect(new Set(keys).size).toBe(3);
  });
});
