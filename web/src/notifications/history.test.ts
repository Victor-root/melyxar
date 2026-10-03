import { describe, expect, it } from "vitest";
import type { Note } from "./api";
import { arrived, marked, NOTHING_HELD, olderPage, removed, unreadAmong } from "./history";
import type { Held } from "./history";

function note(id: string, read = false): Note {
  return {
    id,
    kind: "message",
    level: "ok",
    data: { kind: "message", title: id, text: "" },
    work_id: null,
    poster: [],
    priority: false,
    mandatory: false,
    sticky: false,
    shown_for_ms: null,
    due_at: null,
    created_at: "2026-10-03T12:00:00Z",
    read,
  };
}

const ids = (held: Held) => held.notes.map((one) => one.id);

describe("an account's history as a page holds it", () => {
  it("puts what arrives in its place by time and counts it unread once", () => {
    let held: Held = { notes: [note("c"), note("a", true)], unread: 1, more: false };
    held = arrived(held, note("b"));
    expect(ids(held)).toEqual(["c", "b", "a"]);
    expect(held.unread).toBe(2);
    held = arrived(held, note("b"));
    expect(held.unread).toBe(2);
  });

  it("counts an arrival older than what is held without showing it", () => {
    const held: Held = { notes: [note("m")], unread: 4, more: true };
    const after = arrived(held, note("c"));
    expect(ids(after)).toEqual(["m"]);
    expect(after.unread).toBe(5);
  });

  it("counts a notification recalled unread only if it was read", () => {
    const held: Held = { notes: [note("a", true), note("b")], unread: 1, more: false };
    expect(arrived(held, note("a")).unread).toBe(2);
    expect(arrived(held, note("b")).unread).toBe(1);
  });

  it("counts what was marked, held or not, and nothing twice", () => {
    const held: Held = { notes: [note("c"), note("b"), note("a", true)], unread: 5, more: true };
    const read = marked(held, ["c", "a", "older"], true);
    expect(read.unread).toBe(3);
    expect(read.notes.every((one) => one.read || one.id === "b")).toBe(true);
    expect(marked(read, ["c"], true).unread).toBe(3);
    expect(marked(read, ["c"], false).unread).toBe(4);
    expect(marked({ ...NOTHING_HELD }, ["x"], true).unread).toBe(0);
  });

  it("takes away what was removed and says when it could not know", () => {
    const held: Held = { notes: [note("b"), note("a", true)], unread: 2, more: true };
    const known = removed(held, ["b", "a"]);
    expect(known.held.notes).toEqual([]);
    expect(known.held.unread).toBe(1);
    expect(known.unknown).toBe(false);
    expect(removed(held, ["older"]).unknown).toBe(true);
  });

  it("names the unread held, all or among some", () => {
    const held: Held = { notes: [note("c"), note("b", true), note("a")], unread: 2, more: false };
    expect(unreadAmong(held)).toEqual(["c", "a"]);
    expect(unreadAmong(held, ["a", "b"])).toEqual(["a"]);
  });

  it("puts an older page after what is held, once", () => {
    const held: Held = { notes: [note("c"), note("b")], unread: 0, more: true };
    const after = olderPage(held, { notifications: [note("b"), note("a")], more: false });
    expect(ids(after)).toEqual(["c", "b", "a"]);
    expect(after.more).toBe(false);
  });
});
