import { describe, expect, it } from "vitest";
import { expectedAt, whatTheyHold } from "./subtitleWatch";

const cue = (start: number, end: number, text = "words") => ({
  start_second: start,
  end_second: end,
  text,
});

describe("what a set of cues holds", () => {
  it("counts them and says where they begin and end", () => {
    const held = whatTheyHold([cue(2, 4), cue(4, 9), cue(10, 12)], true);
    expect(held).toMatchObject({ cues: 3, first_start_second: 2, last_end_second: 12, hidden: true });
  });

  it("counts the ones out of order and the ones that end before they begin", () => {
    const held = whatTheyHold([cue(10, 12), cue(2, 4), cue(5, 5), cue(6, 5)], false);
    expect(held.out_of_order).toBe(1);
    expect(held.empty_or_backwards).toBe(2);
  });

  it("has nothing to say of nothing", () => {
    expect(whatTheyHold([], false)).toMatchObject({ cues: 0, first_start_second: null, last_end_second: null });
  });
});

describe("what the times of the file put on the screen", () => {
  it("is the cue whose stretch holds the moment, the end excluded", () => {
    const cues = [cue(0, 3), cue(3, 6), cue(10, 12)];
    expect(expectedAt(cues, 3)).toEqual([cues[1]]);
    expect(expectedAt(cues, 8)).toEqual([]);
    expect(expectedAt(cues, 11.9)).toEqual([cues[2]]);
  });
});
