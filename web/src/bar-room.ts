/*
 * How much of the top of the screen the bar covers right now: all of its band
 * while it is out, none once it has stepped aside, and none where there is no
 * bar. What puts something at the top of the screen reads it, so that thing
 * lands under the bar rather than behind it.
 *
 * Held here rather than written on the page as a property of its style. A
 * property written on the root is inherited by every element of the page, so
 * the browser styled the whole page again each time the bar stepped aside or
 * came back, every card of a library of thousands included, in the middle of
 * the scroll that moved it. Nothing in the stylesheet ever read it.
 */

let out: boolean | null = null;

/** Said by the bar: whether it is out, or null once it is gone. */
export function sayTheBarIsOut(shown: boolean | null): void {
  out = shown;
}

/** The room the bar takes at the top of the screen right now, in pixels. */
export function barRoom(): number {
  if (!out) {
    return 0;
  }
  return parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--header-height")) || 0;
}
