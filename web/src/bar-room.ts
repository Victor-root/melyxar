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

/** Said by the bar: whether it is out, or null once it is gone. Said to the
 *  stylesheet too, by a mark on the root, for what moves aside with the bar:
 *  asked of the page instead, by a rule for a page holding a bar that is
 *  away, the rule was checked again at every change anywhere in the page. */
export function sayTheBarIsOut(shown: boolean | null): void {
  out = shown;
  document.documentElement.toggleAttribute("data-bar-away", shown === false);
}

/** The room the bar takes at the top of the screen right now, in pixels. */
export function barRoom(): number {
  if (!out) {
    return 0;
  }
  return parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--header-height")) || 0;
}

/** Where the piece at the left end of the bar stops and how wide the one at
 *  the right end is, as the heads rising into its band read them. */
interface PiecesEnd {
  startEnd: string;
  sideWidth: string;
}

let piecesEnd: PiecesEnd | null = null;
const heads = new Set<HTMLElement>();

function wear(head: HTMLElement): void {
  if (piecesEnd) {
    head.style.setProperty("--header-start-end", piecesEnd.startEnd);
    head.style.setProperty("--header-side-width", piecesEnd.sideWidth);
  } else {
    head.style.removeProperty("--header-start-end");
    head.style.removeProperty("--header-side-width");
  }
}

/** Said by the bar each time its pieces change size, or null where nothing
 *  rises into its band. Written on the heads alone: written on the root, it
 *  had every element of the page styled again after each change of page. */
export function sayWherePiecesEnd(said: PiecesEnd | null): void {
  piecesEnd = said;
  heads.forEach(wear);
}

/** Given as the ref of a head that rises into the band of the bar: it wears
 *  where the pieces end from the moment it is drawn. */
export function clearOfTheBar(head: HTMLElement | null): (() => void) | undefined {
  if (!head) {
    return undefined;
  }
  heads.add(head);
  wear(head);
  return () => {
    heads.delete(head);
  };
}
