/*
 * The banner's one piece of arithmetic.
 *
 * How many lines of synopsis fit is decided from two measurements taken off
 * the page, and a banner drawn with one line too few looks exactly like a
 * banner whose synopsis was short. Nothing on the screen tells the two apart,
 * which is what this is here for.
 */

import { describe, expect, it } from "vitest";
import { linesThatFit } from "./hero";

/** The height of one line as the banner really computes it: fourteen and a
 *  half points of text on a line and a half of it. */
const LINE = 14.5 * 1.55;

describe("linesThatFit", () => {
  it("counts whole lines and keeps the remainder out", () => {
    expect(linesThatFit(LINE * 3, LINE)).toBe(3);
    expect(linesThatFit(LINE * 3 + LINE * 0.9, LINE)).toBe(3);
    expect(linesThatFit(LINE * 4 - 0.5, LINE)).toBe(3);
  });

  it("draws one line rather than none when only one fits", () => {
    expect(linesThatFit(LINE, LINE)).toBe(1);
    expect(linesThatFit(LINE * 1.9, LINE)).toBe(1);
  });

  it("draws none at all when not even one fits", () => {
    expect(linesThatFit(LINE - 0.5, LINE)).toBe(0);
    expect(linesThatFit(0, LINE)).toBe(0);
    /* A block whose other parts are already taller than the banner: the sum
       comes out below zero, and the answer is still none rather than a
       negative count handed to the stylesheet. */
    expect(linesThatFit(-120, LINE)).toBe(0);
  });

  it("stops at a dozen however tall the banner is", () => {
    expect(linesThatFit(LINE * 12, LINE)).toBe(12);
    expect(linesThatFit(LINE * 40, LINE)).toBe(12);
  });

  it("falls back to one line when the page gives no line height", () => {
    /* What a browser hands back for a line height it has not worked out yet.
       One line is drawn rather than none, and the next measurement, once the
       page has settled, replaces it. */
    expect(linesThatFit(400, Number.NaN)).toBe(1);
    expect(linesThatFit(400, 0)).toBe(1);
  });
});
