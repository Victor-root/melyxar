import { describe, expect, it } from "vitest";
import type { Note, NoteSettings } from "./api";
import { clockOf, isQuiet, minuteOfClock, sayNote, showsNow } from "./wording";

/** Says the key and its values, so a test reads what was chosen. */
const t = (key: string, values?: Record<string, string | number>) =>
  values ? `${key} ${JSON.stringify(values)}` : key;

function a(data: Note["data"], changes: Partial<Note> = {}): Note {
  return {
    id: "n",
    kind: data?.kind ?? "message",
    level: "news",
    data,
    work_id: "w",
    poster: [],
    priority: false,
    mandatory: false,
    sticky: false,
    shown_for_ms: null,
    due_at: null,
    created_at: "2026-10-03T12:00:00Z",
    read: false,
    ...changes,
  };
}

const content = (films: string[], series: { id: string; title: string; episodes: number }[] = []) =>
  a({
    kind: "new_content",
    library: "lib",
    library_name: "Films",
    films: films.length,
    film_titles: films.slice(0, 3),
    series,
  });

describe("what a notification says", () => {
  it("names a single film and opens it", () => {
    expect(sayNote(content(["Amber Field"]), t, "en")).toEqual({
      title: 'notes.new_film {"title":"Amber Field"}',
      detail: "Films",
      to: "/work/w",
    });
  });

  it("counts several films, names a few, and opens the library", () => {
    const said = sayNote(content(["One", "Two", "Three", "Four", "Five"]), t, "en");
    expect(said.title).toBe('notes.new_films {"count":5}');
    expect(said.detail).toBe('notes.and_more {"named":"One, Two, Three","count":2}');
    expect(said.to).toBe("/library/lib");
  });

  it("counts the episodes of one series and opens the series", () => {
    const said = sayNote(content([], [{ id: "s", title: "Salt Road", episodes: 40 }]), t, "en");
    expect(said.title).toBe('notes.new_episodes {"count":40,"series":"Salt Road"}');
    expect(said.to).toBe("/work/s");
    const one = sayNote(content([], [{ id: "s", title: "Salt Road", episodes: 1 }]), t, "en");
    expect(one.title).toBe('notes.new_episode {"series":"Salt Road"}');
  });

  it("adds up films and episodes of several series", () => {
    const said = sayNote(
      content(["Amber Field"], [
        { id: "s", title: "Salt Road", episodes: 3 },
        { id: "u", title: "Tides", episodes: 2 },
      ]),
      t,
      "en",
    );
    expect(said.title).toBe('notes.new_many {"count":6,"library":"Films"}');
    expect(said.detail).toBe("Salt Road, Tides, Amber Field");
  });

  it("says what a deletion checked on the disk", () => {
    const deleted = (checked_gone: number | null) =>
      sayNote(a({ kind: "deletion", titles: ["X"], works: 1, checked_gone }), t, "en");
    expect(deleted(null).title).toBe("delete.done_library");
    expect(deleted(0).detail).toBe("delete.done_disk_none");
    expect(deleted(1).detail).toBe("delete.done_disk_checked_one");
    expect(deleted(4).detail).toBe('delete.done_disk_checked {"count":4}');
  });

  it("keeps the administrator's words as written", () => {
    expect(sayNote(a({ kind: "message", title: "Hello", text: "" }), t, "en")).toEqual({
      title: "Hello",
      detail: null,
      to: null,
    });
  });

  it("says something even of what it cannot read", () => {
    expect(sayNote(a(null), t, "en").title).toBe("notes.unreadable");
  });
});

const settings = (changes: Partial<NoteSettings> = {}): NoteSettings => ({
  quiet_from: null,
  quiet_until: null,
  priority_while_playing: true,
  priority_while_quiet: true,
  ...changes,
});

describe("quiet hours", () => {
  it("run across midnight when they end before they start", () => {
    const night = settings({ quiet_from: 22 * 60, quiet_until: 7 * 60 });
    expect(isQuiet(23 * 60, night)).toBe(true);
    expect(isQuiet(3 * 60, night)).toBe(true);
    expect(isQuiet(7 * 60, night)).toBe(false);
    expect(isQuiet(12 * 60, night)).toBe(false);
  });

  it("hold within a day otherwise, and never when unset", () => {
    const afternoon = settings({ quiet_from: 13 * 60, quiet_until: 15 * 60 });
    expect(isQuiet(14 * 60, afternoon)).toBe(true);
    expect(isQuiet(15 * 60, afternoon)).toBe(false);
    expect(isQuiet(14 * 60, settings())).toBe(false);
  });
});

describe("what shows on the screen", () => {
  const plain = { priority: false, mandatory: false };
  const urgent = { priority: true, mandatory: false };
  const mandatory = { priority: false, mandatory: true };

  it("shows anything when nothing is playing and nobody is asleep", () => {
    expect(showsNow(plain, settings(), false, 12 * 60)).toBe(true);
  });

  it("shows only what is urgent during a film, and only if the account wants it", () => {
    expect(showsNow(plain, settings(), true, 12 * 60)).toBe(false);
    expect(showsNow(urgent, settings(), true, 12 * 60)).toBe(true);
    expect(showsNow(mandatory, settings(), true, 12 * 60)).toBe(true);
    expect(showsNow(urgent, settings({ priority_while_playing: false }), true, 12 * 60)).toBe(false);
  });

  it("shows only what is urgent during quiet hours, and only if the account wants it", () => {
    const night = settings({ quiet_from: 22 * 60, quiet_until: 7 * 60 });
    expect(showsNow(plain, night, false, 23 * 60)).toBe(false);
    expect(showsNow(urgent, night, false, 23 * 60)).toBe(true);
    expect(showsNow(urgent, { ...night, priority_while_quiet: false }, false, 23 * 60)).toBe(false);
  });
});

describe("the clock of quiet hours", () => {
  it("writes and reads a minute of the day", () => {
    expect(clockOf(22 * 60 + 5)).toBe("22:05");
    expect(clockOf(0)).toBe("00:00");
    expect(minuteOfClock("07:30")).toBe(7 * 60 + 30);
    expect(minuteOfClock(clockOf(1439))).toBe(1439);
  });

  it("reads nothing from what is not a time", () => {
    expect(minuteOfClock("")).toBeNull();
    expect(minuteOfClock("24:00")).toBeNull();
    expect(minuteOfClock("7h30")).toBeNull();
  });
});
