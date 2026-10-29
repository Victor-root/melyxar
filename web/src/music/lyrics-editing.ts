/*
 * The arithmetic of the lyrics editor: the lines being written, read from the
 * lyrics a song has and from text pasted in, moved, stamped, and turned back
 * into what the server keeps.
 *
 * Moments are in milliseconds and kept to the hundredth of a second, which is
 * as fine as the server writes them.
 */

import type { LyricLine, SongLyrics } from "./api";

/** One line in the editor. Its key stays with it whatever is done to it. */
export interface EditedLine {
  key: number;
  /** When it is sung, or nothing yet. */
  at: number | null;
  text: string;
}

let lastKey = 0;

/** A line the editor has not seen before. */
export function aLine(at: number | null, text: string): EditedLine {
  lastKey += 1;
  return { key: lastKey, at, text };
}

/** A moment kept to the hundredth of a second, never before the start. */
export function toHundredth(ms: number): number {
  return Math.max(0, Math.round(ms / 10) * 10);
}

/** The lines of the lyrics a song has: stamped when they are, else one per
 *  line of the words, the empty ones between verses kept. */
export function linesOf(lyrics: SongLyrics | null): EditedLine[] {
  if (!lyrics) {
    return [];
  }
  if (lyrics.lines.length > 0) {
    return lyrics.lines.map((line) => aLine(toHundredth(line.at_ms), line.text));
  }
  return wordsOf(lyrics.plain);
}

function wordsOf(plain: string): EditedLine[] {
  const lines = plain.replace(/\r/g, "").split("\n").map((text) => text.trim());
  while (lines.length > 0 && lines[0] === "") {
    lines.shift();
  }
  while (lines.length > 0 && lines[lines.length - 1] === "") {
    lines.pop();
  }
  return lines.map((text) => aLine(null, text));
}

/** A moment as it is written in the form every player reads: 01:05.34. */
export function stampOf(ms: number): string {
  const hundredths = Math.round(Math.max(0, ms) / 10);
  const minutes = Math.floor(hundredths / 6000);
  const seconds = Math.floor(hundredths / 100) % 60;
  const rest = hundredths % 100;
  return `${minutes < 10 ? "0" : ""}${minutes}:${seconds < 10 ? "0" : ""}${seconds}.${rest < 10 ? "0" : ""}${rest}`;
}

/** A moment typed by hand: "1:05.34", "1:05", "65.3". Nothing for anything
 *  else, which the field then leaves as it was. */
export function readStamp(text: string): number | null {
  const found = /^\s*(?:(\d+):)?(\d+(?:[.,]\d+)?)\s*$/.exec(text);
  if (!found) {
    return null;
  }
  const minutes = found[1] === undefined ? 0 : Number(found[1]);
  const seconds = Number(found[2].replace(",", "."));
  if (found[1] !== undefined && seconds >= 60) {
    return null;
  }
  return toHundredth((minutes * 60 + seconds) * 1000);
}

/** The lines written as text, each after its moment when it has one. */
export function asText(lines: readonly EditedLine[]): string {
  return lines.map((line) => (line.at === null ? line.text : `[${stampOf(line.at)}]${line.text}`)).join("\n");
}

const STAMP = /^\[(\d+):(\d{1,2})(?:[.:](\d{1,3}))?\]/;
const HEADING = /^\[(ar|ti|al|au|by|length|re|ve|tool|#)\s*:[^\]]*\]\s*$/i;
const OFFSET = /^\[offset\s*:\s*([+-]?\d+)\s*\]\s*$/i;

/** Lines read back from text, stamped or not: what the text form above
 *  writes, and what a file of lyrics holds. A line sung at several moments
 *  comes once at each; the headings of the form are left out, and its
 *  offset, which says how much sooner every line comes, is applied. */
export function fromText(text: string): EditedLine[] {
  let offset = 0;
  const read: { at: number | null; text: string }[] = [];
  for (const raw of text.replace(/\r/g, "").split("\n")) {
    const line = raw.trim();
    const offsetFound = OFFSET.exec(line);
    if (offsetFound) {
      offset = Number(offsetFound[1]);
      continue;
    }
    if (HEADING.test(line)) {
      continue;
    }
    const stamps: number[] = [];
    let rest = line;
    for (let found = STAMP.exec(rest); found; found = STAMP.exec(rest)) {
      const fraction = (found[3] ?? "").padEnd(3, "0");
      stamps.push(Number(found[1]) * 60_000 + Number(found[2]) * 1000 + Number(fraction));
      rest = rest.slice(found[0].length);
    }
    if (stamps.length === 0) {
      read.push({ at: null, text: line });
    }
    for (const at of stamps) {
      read.push({ at, text: rest.trim() });
    }
  }
  // Stamped throughout, they are put in the order they are sung, the gaps
  // between verses left out; else they stay where they were written.
  const stamped = read.filter((line) => line.at !== null);
  const lines =
    stamped.length > 0 && !read.some((line) => line.at === null && line.text !== "")
      ? stamped.sort((one, other) => (one.at ?? 0) - (other.at ?? 0))
      : read;
  return wordsTrimmed(lines).map((line) => aLine(line.at === null ? null : toHundredth(line.at - offset), line.text));
}

/** Without the empty lines at either end, which say nothing. */
function wordsTrimmed<T extends { at: number | null; text: string }>(lines: T[]): T[] {
  const kept = (line: T) => line.text !== "" || line.at !== null;
  const first = lines.findIndex(kept);
  if (first < 0) {
    return [];
  }
  let last = lines.length - 1;
  while (!kept(lines[last])) {
    last -= 1;
  }
  return lines.slice(first, last + 1);
}

/** Whether a line waits for a moment: one with words and none yet. An empty
 *  line without one is the gap between two verses, which needs none. */
export function waitsForAMoment(line: EditedLine): boolean {
  return line.at === null && line.text.trim() !== "";
}

/** The keys of the lines stamped sooner than a line above them, which the
 *  song would light out of turn. */
export function outOfOrder(lines: readonly EditedLine[]): Set<number> {
  const found = new Set<number>();
  let latest = -1;
  for (const line of lines) {
    if (line.at === null) {
      continue;
    }
    if (line.at < latest) {
      found.add(line.key);
    }
    latest = Math.max(latest, line.at);
  }
  return found;
}

/** The line being sung `ms` into the song: the stamped line whose moment has
 *  come last, or -1 before the first. */
export function sungAt(lines: readonly EditedLine[], ms: number): number {
  let found = -1;
  let latest = -1;
  lines.forEach((line, index) => {
    if (line.at !== null && line.at <= ms && line.at >= latest) {
      found = index;
      latest = line.at;
    }
  });
  return found;
}

/** The lines from `from` on moved later by `by`, sooner when negative, none
 *  before the start of the song. */
export function shifted(lines: readonly EditedLine[], from: number, by: number): EditedLine[] {
  return lines.map((line, index) =>
    index < from || line.at === null ? line : { ...line, at: toHundredth(line.at + by) },
  );
}

/** The next line after `index` that waits to be stamped as the song plays:
 *  the gaps between verses are passed over. -1 past the last. */
export function nextToStamp(lines: readonly EditedLine[], index: number): number {
  for (let next = index + 1; next < lines.length; next += 1) {
    if (lines[next].text.trim() !== "" || lines[next].at !== null) {
      return next;
    }
  }
  return -1;
}

/** The line stamped before `index` as the song plays, the gaps between
 *  verses passed over: the one to go back to. -1 before the first. */
export function previousToStamp(lines: readonly EditedLine[], index: number): number {
  for (let before = Math.min(index, lines.length) - 1; before >= 0; before -= 1) {
    if (lines[before].text.trim() !== "" || lines[before].at !== null) {
      return before;
    }
  }
  return -1;
}

/** What the server keeps of the lines: stamped lines when every line with
 *  words has a moment, the gaps between verses that have none left out;
 *  plain words when no line has one; nothing while only some have. */
export type Kept = { lines: LyricLine[]; plain: "" } | { lines: []; plain: string } | null;

export function toKeep(lines: readonly EditedLine[]): Kept {
  const stamped = lines.filter((line) => line.at !== null);
  if (stamped.length === 0) {
    const plain = lines.map((line) => line.text.trim()).join("\n").trim();
    return plain === "" ? null : { lines: [], plain };
  }
  if (lines.some(waitsForAMoment)) {
    return null;
  }
  return {
    lines: stamped
      .map((line) => ({ at_ms: line.at ?? 0, text: line.text.trim() }))
      .sort((one, other) => one.at_ms - other.at_ms),
    plain: "",
  };
}

/** The lines as the server keeps them once stamped: in the order they are
 *  sung, the gaps between verses without a moment left out. Plain words,
 *  and lines only some of which are stamped, stay as they are. */
export function inSungOrder(lines: readonly EditedLine[]): EditedLine[] {
  if (!lines.some((line) => line.at !== null) || lines.some(waitsForAMoment)) {
    return [...lines];
  }
  return lines.filter((line) => line.at !== null).sort((one, other) => (one.at ?? 0) - (other.at ?? 0));
}

/** Whether two sets of lines say the same, whatever their keys. */
export function sameLines(one: readonly EditedLine[], other: readonly EditedLine[]): boolean {
  return (
    one.length === other.length &&
    one.every((line, index) => line.at === other[index].at && line.text === other[index].text)
  );
}

/** The lines being written, and the steps back and forward through what was
 *  done to them. */
export interface Editing {
  lines: EditedLine[];
  past: EditedLine[][];
  future: EditedLine[][];
  /** What the last step was doing, when the next one of the same is the
      same step: a word typed after another, a mark dragged further. */
  merging: string | null;
}

export type EditingChange =
  | { kind: "start"; lines: EditedLine[] }
  | { kind: "edit"; edit: (lines: EditedLine[]) => EditedLine[]; merging?: string }
  | { kind: "settle" }
  | { kind: "undo" }
  | { kind: "redo" };

/** How many steps back are kept. */
const MOST_STEPS = 300;

export const NOTHING_EDITED: Editing = { lines: [], past: [], future: [], merging: null };

export function edited(state: Editing, change: EditingChange): Editing {
  switch (change.kind) {
    case "start":
      return { ...NOTHING_EDITED, lines: change.lines };
    case "edit": {
      const lines = change.edit(state.lines);
      if (lines === state.lines) {
        return state;
      }
      if (change.merging !== undefined && change.merging === state.merging) {
        return { ...state, lines, future: [] };
      }
      return {
        lines,
        past: [...state.past, state.lines].slice(-MOST_STEPS),
        future: [],
        merging: change.merging ?? null,
      };
    }
    case "settle":
      return state.merging === null ? state : { ...state, merging: null };
    case "undo": {
      const back = state.past[state.past.length - 1];
      return back === undefined
        ? state
        : { lines: back, past: state.past.slice(0, -1), future: [state.lines, ...state.future], merging: null };
    }
    case "redo": {
      const on = state.future[0];
      return on === undefined
        ? state
        : { lines: on, past: [...state.past, state.lines], future: state.future.slice(1), merging: null };
    }
  }
}
