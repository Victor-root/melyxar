import { describe, expect, it } from "vitest";
import type { SongLyrics } from "./api";
import {
  NOTHING_EDITED,
  aLine,
  asText,
  edited,
  endedAt,
  inSungOrder,
  withABreakAfter,
  fromText,
  linesOf,
  nextToStamp,
  outOfOrder,
  previousToStamp,
  readStamp,
  sameLines,
  shifted,
  stampOf,
  sungAt,
  toHundredth,
  toKeep,
} from "./lyrics-editing";
import type { EditedLine, Editing } from "./lyrics-editing";

const said = (lines: EditedLine[]) => lines.map((line) => [line.at, line.text]);

const lyrics = (lines: SongLyrics["lines"], plain = ""): SongLyrics => ({
  source: "song",
  plain,
  lines,
  instrumental: false,
  synchronised: null,
  offset_ms: 0,
});

describe("linesOf", () => {
  it("takes stamped lines with their moments, to the hundredth", () => {
    const read = linesOf(lyrics([{ at_ms: 7_784, text: "First" }, { at_ms: 12_500, text: "" }]));
    expect(said(read)).toEqual([[7_780, "First"], [12_500, ""]]);
  });

  it("takes plain words line by line, the gaps between verses kept and none at either end", () => {
    const read = linesOf(lyrics([], "\n  One \nTwo\n\nThree\n\n"));
    expect(said(read)).toEqual([[null, "One"], [null, "Two"], [null, ""], [null, "Three"]]);
  });

  it("gives every line a key of its own", () => {
    const read = linesOf(lyrics([], "Same\nSame"));
    expect(read[0].key).not.toBe(read[1].key);
    expect(linesOf(null)).toEqual([]);
  });
});

describe("stamps", () => {
  it("writes a moment as minutes, seconds and hundredths", () => {
    expect(stampOf(0)).toBe("00:00.00");
    expect(stampOf(65_340)).toBe("01:05.34");
    expect(stampOf(59_996)).toBe("01:00.00");
    expect(stampOf(3_725_430)).toBe("62:05.43");
  });

  it("reads a moment typed in any of the usual ways", () => {
    expect(readStamp("1:05.34")).toBe(65_340);
    expect(readStamp(" 01:05 ")).toBe(65_000);
    expect(readStamp("65,3")).toBe(65_300);
    expect(readStamp("0:59.999")).toBe(60_000);
  });

  it("reads nothing from a slip", () => {
    expect(readStamp("1:75")).toBeNull();
    expect(readStamp("abc")).toBeNull();
    expect(readStamp("")).toBeNull();
    expect(readStamp("-3")).toBeNull();
  });

  it("keeps a moment to the hundredth and never before the start", () => {
    expect(toHundredth(1_234.6)).toBe(1_230);
    expect(toHundredth(-40)).toBe(0);
  });
});

describe("text", () => {
  it("reads back what it writes", () => {
    const lines = [aLine(1_000, "One"), aLine(null, "Two"), aLine(4_500, "")];
    expect(asText(lines)).toBe("[00:01.00]One\nTwo\n[00:04.50]");
    expect(said(fromText(asText(lines)))).toEqual(said(lines));
  });

  it("reads a file of lyrics: headings left out, offset applied, a line sung twice at both moments", () => {
    const read = fromText("[ar:Amber Field]\n[offset:+500]\n[00:30.00][00:10.00]Chorus\n\n[00:20.5]Verse\n");
    expect(said(read)).toEqual([[9_500, "Chorus"], [20_000, "Verse"], [29_500, "Chorus"]]);
  });

  it("keeps plain words where they are, among stamped ones", () => {
    const read = fromText("\n[00:02.00]Stamped\nNot yet\n\nAfter a gap\n");
    expect(said(read)).toEqual([[2_000, "Stamped"], [null, "Not yet"], [null, ""], [null, "After a gap"]]);
  });

  it("keeps brackets that are not moments in the words", () => {
    expect(said(fromText("[Chorus]\n[00:05.00][softly] here"))).toEqual([
      [null, "[Chorus]"],
      [5_000, "[softly] here"],
    ]);
  });
});

describe("outOfOrder", () => {
  it("finds the lines stamped sooner than one above them, past the lines without a moment", () => {
    const lines = [aLine(1_000, "a"), aLine(5_000, "b"), aLine(null, "c"), aLine(3_000, "d"), aLine(6_000, "e")];
    expect([...outOfOrder(lines)]).toEqual([lines[3].key]);
  });
});

describe("sungAt", () => {
  it("is the stamped line whose moment came last", () => {
    const lines = [aLine(1_000, "a"), aLine(null, "b"), aLine(5_000, "c")];
    expect(sungAt(lines, 500)).toBe(-1);
    expect(sungAt(lines, 4_999)).toBe(0);
    expect(sungAt(lines, 5_000)).toBe(2);
  });
});

describe("shifted", () => {
  it("moves the lines from one on, never before the start", () => {
    const lines = [aLine(1_000, "a"), aLine(null, "b"), aLine(2_000, "c")];
    expect(said(shifted(lines, 0, -1_500))).toEqual([[0, "a"], [null, "b"], [500, "c"]]);
    expect(said(shifted(lines, 2, 250))).toEqual([[1_000, "a"], [null, "b"], [2_250, "c"]]);
  });
});

describe("endedAt", () => {
  const song = 10_000;

  it("moves the start of the next line, later or sooner, never so either lasts less than the shortest", () => {
    const lines = [aLine(1_000, "a"), aLine(3_000, "b"), aLine(6_000, "c")];
    expect(said(endedAt(lines, lines[0].key, 4_000, song))).toEqual([[1_000, "a"], [4_000, "b"], [6_000, "c"]]);
    expect(said(endedAt(lines, lines[0].key, 2_000, song))).toEqual([[1_000, "a"], [2_000, "b"], [6_000, "c"]]);
    expect(said(endedAt(lines, lines[0].key, 7_000, song))).toEqual([[1_000, "a"], [5_900, "b"], [6_000, "c"]]);
    expect(said(endedAt(lines, lines[0].key, 500, song))).toEqual([[1_000, "a"], [1_100, "b"], [6_000, "c"]]);
  });

  it("never adds a line, a break included", () => {
    const lines = [aLine(1_000, "a"), aLine(3_000, "b")];
    expect(endedAt(lines, lines[0].key, 2_000, song)).toHaveLength(2);
  });

  it("moves a break like any other line", () => {
    const lines = [aLine(1_000, "a"), aLine(2_000, ""), aLine(3_000, "b")];
    expect(said(endedAt(lines, lines[0].key, 1_500, song))[1]).toEqual([1_500, ""]);
    expect(said(endedAt(lines, lines[1].key, 2_500, song))[2]).toEqual([2_500, "b"]);
  });

  it("leaves the last line, lines without a moment, and moves that change nothing, as they are", () => {
    const last = [aLine(1_000, "a")];
    expect(endedAt(last, last[0].key, 4_000, song)).toBe(last);
    const lines = [aLine(null, "a"), aLine(3_000, "b")];
    expect(endedAt(lines, lines[0].key, 2_000, song)).toBe(lines);
    const stamped = [aLine(1_000, "a"), aLine(3_000, "b")];
    expect(endedAt(stamped, stamped[0].key, 3_000, song)).toBe(stamped);
  });
});

describe("withABreakAfter", () => {
  const song = 60_000;

  it("puts a break where the line ends, the next line starting five seconds later and no other moving", () => {
    const lines = [aLine(1_000, "a"), aLine(4_000, "b"), aLine(15_000, "c")];
    const pause = aLine(null, "");
    const done = withABreakAfter(lines, lines[0].key, song, pause);
    expect(said(done)).toEqual([[1_000, "a"], [4_000, ""], [9_000, "b"], [15_000, "c"]]);
    expect(done[1].key).toBe(pause.key);
  });

  it("leaves the next line the shortest it may last when it lasts less than the break", () => {
    const lines = [aLine(1_000, "a"), aLine(4_000, "b"), aLine(6_000, "c")];
    expect(said(withABreakAfter(lines, lines[0].key, song))).toEqual([[1_000, "a"], [4_000, ""], [5_900, "b"], [6_000, "c"]]);
  });

  it("keeps the order of the list", () => {
    const lines = [aLine(1_000, "a"), aLine(4_000, "b"), aLine(null, "c"), aLine(20_000, "d")];
    expect(said(withABreakAfter(lines, lines[1].key, song))).toEqual([
      [1_000, "a"],
      [4_000, "b"],
      [20_000, ""],
      [null, "c"],
      [25_000, "d"],
    ]);
  });

  it("comes five seconds after the last line starts, before the end of the song", () => {
    const lines = [aLine(1_000, "a")];
    expect(said(withABreakAfter(lines, lines[0].key, song))).toEqual([[1_000, "a"], [6_000, ""]]);
    expect(said(withABreakAfter(lines, lines[0].key, 3_000))).toEqual([[1_000, "a"], [2_900, ""]]);
  });

  it("leaves a line without a moment, and one with no room after it, as they are", () => {
    const lines = [aLine(null, "a"), aLine(59_950, "b")];
    expect(withABreakAfter(lines, lines[0].key, song)).toBe(lines);
    expect(withABreakAfter(lines, lines[1].key, song)).toBe(lines);
    const packed = [aLine(1_000, "a"), aLine(2_000, "b"), aLine(2_100, "c")];
    expect(withABreakAfter(packed, packed[0].key, song)).toBe(packed);
  });
});

describe("nextToStamp", () => {
  it("passes over the gaps between verses", () => {
    const lines = [aLine(null, "a"), aLine(null, ""), aLine(null, "b"), aLine(3_000, "")];
    expect(nextToStamp(lines, -1)).toBe(0);
    expect(nextToStamp(lines, 0)).toBe(2);
    expect(nextToStamp(lines, 2)).toBe(3);
    expect(nextToStamp(lines, 3)).toBe(-1);
    expect(previousToStamp(lines, 2)).toBe(0);
    expect(previousToStamp(lines, 0)).toBe(-1);
    expect(previousToStamp(lines, 9)).toBe(3);
  });
});

describe("toKeep", () => {
  it("keeps stamped lines in the order they are sung, the gaps without a moment left out", () => {
    const lines = [aLine(4_000, " b "), aLine(null, ""), aLine(1_000, "a"), aLine(6_000, "")];
    expect(toKeep(lines)).toEqual({
      lines: [
        { at_ms: 1_000, text: "a" },
        { at_ms: 4_000, text: "b" },
        { at_ms: 6_000, text: "" },
      ],
      plain: "",
    });
  });

  it("keeps plain words when no line has a moment", () => {
    expect(toKeep([aLine(null, "a"), aLine(null, ""), aLine(null, "b ")])).toEqual({ lines: [], plain: "a\n\nb" });
  });

  it("keeps nothing while some lines with words wait for a moment, or when there are no words", () => {
    expect(toKeep([aLine(1_000, "a"), aLine(null, "b")])).toBeNull();
    expect(toKeep([aLine(null, " "), aLine(null, "")])).toBeNull();
    expect(toKeep([])).toBeNull();
  });
});

describe("inSungOrder", () => {
  it("puts stamped lines in the order they are sung, the gaps without a moment left out", () => {
    const lines = [aLine(4_000, "b"), aLine(null, ""), aLine(1_000, "a"), aLine(6_000, "")];
    expect(said(inSungOrder(lines))).toEqual([[1_000, "a"], [4_000, "b"], [6_000, ""]]);
  });

  it("leaves plain words and half stamped lines where they are", () => {
    const plain = [aLine(null, "b"), aLine(null, "a")];
    expect(said(inSungOrder(plain))).toEqual(said(plain));
    const half = [aLine(4_000, "b"), aLine(null, "a")];
    expect(said(inSungOrder(half))).toEqual(said(half));
  });
});

describe("sameLines", () => {
  it("compares what the lines say, whatever their keys", () => {
    expect(sameLines([aLine(1, "a")], [aLine(1, "a")])).toBe(true);
    expect(sameLines([aLine(1, "a")], [aLine(2, "a")])).toBe(false);
    expect(sameLines([aLine(1, "a")], [])).toBe(false);
  });
});

describe("edited", () => {
  const start = edited(NOTHING_EDITED, { kind: "start", lines: [aLine(null, "a")] });
  const typed = (state: Editing, text: string) =>
    edited(state, { kind: "edit", edit: (lines) => [{ ...lines[0], text }], merging: "text" });

  it("goes back and forward a step at a time", () => {
    const once = edited(start, { kind: "edit", edit: (lines) => [...lines, aLine(1_000, "b")] });
    const back = edited(once, { kind: "undo" });
    expect(back.lines).toBe(start.lines);
    expect(edited(back, { kind: "redo" }).lines).toBe(once.lines);
    expect(edited(start, { kind: "undo" })).toBe(start);
  });

  it("makes one step of what is done the same way in a row, until it settles", () => {
    const words = typed(typed(start, "ab"), "abc");
    expect(words.past).toHaveLength(1);
    expect(edited(words, { kind: "undo" }).lines[0].text).toBe("a");
    const settled = typed(edited(words, { kind: "settle" }), "abcd");
    expect(settled.past).toHaveLength(2);
  });

  it("forgets the steps forward once something else is done, and keeps nothing that changed nothing", () => {
    const back = edited(typed(start, "b"), { kind: "undo" });
    expect(typed(back, "c").future).toEqual([]);
    expect(edited(start, { kind: "edit", edit: (lines) => lines })).toBe(start);
  });
});
