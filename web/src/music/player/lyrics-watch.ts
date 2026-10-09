/*
 * What the page tells the journal of the lyrics it shows: the words it was
 * handed, each line it lights and where the song was then, and the two ways
 * following a song can go wrong without anything throwing: the line lit
 * is not the one the song is at, and the clock of the song stands still.
 *
 * The journal writes the facts itself; this only picks which one to send.
 * Each is bounded per song so a long one cannot fill the journal.
 */

import { music } from "../api";
import type { LyricLine, SongLyrics } from "../api";

/** The most lines said to be lit, mismatches and stopped clocks per song. */
const MOST_LINES_LIT = 300;
const MOST_TROUBLES = 20;

/** What is odd about the lines handed over: how many are not in the order
    they are sung in (which breaks the search for the line being sung) and how
    many have nothing in them. */
export function describeLines(lines: readonly LyricLine[]): {
  out_of_order: number;
  empty_lines: number;
  first_at_ms: number | null;
  last_at_ms: number | null;
} {
  let outOfOrder = 0;
  for (let index = 1; index < lines.length; index += 1) {
    if (lines[index].at_ms < lines[index - 1].at_ms) {
      outOfOrder += 1;
    }
  }
  return {
    out_of_order: outOfOrder,
    empty_lines: lines.filter((line) => line.text.trim() === "").length,
    first_at_ms: lines.length > 0 ? lines[0].at_ms : null,
    last_at_ms: lines.length > 0 ? lines[lines.length - 1].at_ms : null,
  };
}

/** Why a line was lit, given the one lit before it, if there was one. */
export function whyLit(before: number | null, now: number): "first" | "played" | "after_a_jump" {
  if (before === null) {
    return "first";
  }
  return now === before + 1 ? "played" : "after_a_jump";
}

/** Tells the journal what the page was handed. */
export function sayLyricsRead(song: string, lyrics: SongLyrics) {
  void music.tellTheJournalOfLyrics({
    song,
    saw: "lyrics_read",
    source: lyrics.source,
    lines: lyrics.lines.length,
    instrumental: lyrics.instrumental,
    ...describeLines(lyrics.lines),
  });
}

/** What is said of one song while it is on show: keeps its own counts. */
export class LyricsReport {
  private lit = 0;
  private troubles = 0;
  private before: number | null = null;

  constructor(
    private readonly song: string,
    private readonly lines: readonly LyricLine[],
  ) {}

  /** A line was lit. */
  lineLit(active: number, positionSeconds: number) {
    const why = whyLit(this.before, active);
    this.before = active;
    if (this.lit >= MOST_LINES_LIT) {
      return;
    }
    this.lit += 1;
    void music.tellTheJournalOfLyrics({
      song: this.song,
      saw: "line_lit",
      why,
      position_ms: Math.round(positionSeconds * 1000),
      active,
      lines: this.lines.length,
      line_at_ms: this.lines[active]?.at_ms ?? null,
      next_at_ms: this.lines[active + 1]?.at_ms ?? null,
      text: this.lines[active]?.text ?? "",
    });
  }

  lineNotLit(positionSeconds: number, expected: number, shown: number) {
    if (this.troubles >= MOST_TROUBLES) {
      return;
    }
    this.troubles += 1;
    void music.tellTheJournalOfLyrics({
      song: this.song,
      saw: "line_not_lit",
      position_ms: Math.round(positionSeconds * 1000),
      expected,
      shown,
      expected_at_ms: this.lines[expected]?.at_ms ?? null,
    });
  }

  clockStoodStill(positionSeconds: number, forMs: number) {
    if (this.troubles >= MOST_TROUBLES) {
      return;
    }
    this.troubles += 1;
    void music.tellTheJournalOfLyrics({
      song: this.song,
      saw: "clock_stood_still",
      position_ms: Math.round(positionSeconds * 1000),
      for_ms: forMs,
    });
  }
}

/** How often the page looks at what it shows against the clock, which is a
    look on a beat of its own so that it still happens when nothing is drawn. */
export const LOOK_EVERY_MS = 2000;

/** What one look comes to, given the last. A pure reading of the numbers, so
    that what counts as trouble can be tested without a page. */
export function lookAt(
  now: { expected: number; shown: number; position: number; playing: boolean },
  before: { mismatches: number; stills: number; position: number | null },
): { mismatches: number; stills: number; position: number; lineNotLit: boolean; clockStill: boolean } {
  const mismatches = now.expected !== now.shown ? before.mismatches + 1 : 0;
  const stills = now.playing && before.position === now.position ? before.stills + 1 : 0;
  return {
    mismatches,
    stills,
    position: now.position,
    // Said when it has been so twice running, and then once a minute at most
    // by the counts being reset: a single look can fall between two redraws.
    lineNotLit: mismatches === 2,
    clockStill: stills === 2,
  };
}
