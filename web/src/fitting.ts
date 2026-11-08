/*
 * How much of a synopsis the top of a page has room for.
 *
 * The page asks three things in turn, each only when the one before failed:
 * does the text fit as it is, does it fit once its column is let out to the
 * edge of the page, and failing both, how many whole lines of it fit, the
 * last line being given to the word that opens the rest. Worked out here on
 * numbers the page measured, so the steps can be checked without a page.
 *
 * The measuring itself is the hook at the end, shared by every page whose top
 * is one screen: the page of a work and the page of a person.
 */

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";

/** Where a synopsis stands. */
export interface Fit {
  /** Let out to the width of its whole column. */
  wide: boolean;
  /** How many lines it is cut to, or nothing when it is shown whole. */
  lines: number | null;
}

/** Where every measuring starts again: as it is, whole. */
export const AS_IT_IS: Fit = { wide: false, lines: null };

/** Fewer lines than this is not a synopsis any more, whatever the room. */
export const FEWEST_LINES = 2;

/** A pixel of slack: a line of text is rarely a whole number of pixels. */
const SLACK = 1;

/**
 * The next step, given where the synopsis stands, how tall it is whole, how
 * much room it has, and how tall one of its lines is. The same step when it is
 * settled, which is what stops the measuring.
 */
export function nextFit(fit: Fit, whole: number, room: number, line: number): Fit {
  if (fit.lines !== null || whole <= room + SLACK) {
    return fit;
  }
  if (!fit.wide) {
    return { wide: true, lines: null };
  }
  // One line of the room goes to the word that opens the rest.
  const lines = line > 0 ? Math.floor(room / line) - 1 : FEWEST_LINES;
  return { wide: true, lines: Math.max(FEWEST_LINES, lines) };
}

/** What a page hands its text to have it fitted. */
export interface Fitted {
  /** A box as tall as the room the top of the page has, drawn but unseen. */
  ruler: React.RefObject<HTMLDivElement | null>;
  /** The column the text stands in, everything else in it included. */
  column: React.RefObject<HTMLDivElement | null>;
  /** The text that gives way. */
  text: React.RefObject<HTMLParagraphElement | null>;
  fit: Fit;
  /** Whether it is cut right now, which is when "more" is offered. */
  cut: boolean;
  /** Whether somebody asked for the whole of it. */
  open: boolean;
  toggle: () => void;
  /** What the text wears to be cut to its lines. */
  style: CSSProperties | undefined;
}

/**
 * Fits a text into the top of a page, measured again from the start whenever
 * the window or anything in `shape` changes: whatever stands above the text
 * and takes its room. Off, the text is shown whole and nothing is offered to
 * cut it.
 */
export function useFittedText(shape: unknown[], enabled = true): Fitted {
  const ruler = useRef<HTMLDivElement>(null);
  const column = useRef<HTMLDivElement>(null);
  const text = useRef<HTMLParagraphElement>(null);
  const [fit, setFit] = useState<Fit>(AS_IT_IS);
  const [open, setOpen] = useState(false);
  const [measured, setMeasured] = useState(0);

  useLayoutEffect(() => {
    setFit(AS_IT_IS);
    setOpen(false);
  }, [measured, enabled, ...shape]);

  useLayoutEffect(() => {
    const room = ruler.current;
    const all = column.current;
    const words = text.current;
    if (!enabled || open || !room || !all || !words) {
      return;
    }
    const rest = all.offsetHeight - words.offsetHeight;
    const line = parseFloat(getComputedStyle(words).lineHeight) || 0;
    const next = nextFit(fit, words.scrollHeight, room.offsetHeight - rest, line);
    if (next !== fit) {
      setFit(next);
    }
  });

  useEffect(() => {
    const again = () => setMeasured((count) => count + 1);
    window.addEventListener("resize", again);
    return () => window.removeEventListener("resize", again);
  }, []);

  const cut = fit.lines !== null && !open;
  return {
    ruler,
    column,
    text,
    fit,
    cut,
    open,
    toggle: () => setOpen((was) => !was),
    style: cut && fit.lines !== null ? { WebkitLineClamp: fit.lines, lineClamp: fit.lines } : undefined,
  };
}

/**
 * As many whole words of `text` as `fits` accepts, followed by an ellipsis,
 * or the text itself when it fits whole. For a text that runs on round a
 * picture, which a cut by lines would hold beside the picture instead.
 */
export function wordsThatFit(text: string, fits: (shown: string) => boolean): string {
  if (fits(text)) {
    return text;
  }
  const words = text.split(/\s+/);
  const cutAfter = (count: number) => `${words.slice(0, count).join(" ").replace(/[\s,;:.]+$/, "")}…`;
  let fitting = 0;
  let tooMany = words.length;
  while (tooMany - fitting > 1) {
    const middle = Math.floor((fitting + tooMany) / 2);
    if (fits(cutAfter(middle))) {
      fitting = middle;
    } else {
      tooMany = middle;
    }
  }
  return fitting === 0 ? "" : cutAfter(fitting);
}

/** How a box standing in a column fills the room the others leave it. */
export interface FilledRoom {
  /** How tall its picture is drawn, in pixels. */
  pictureHeight: number;
  /** The words of its text that fit, or nothing when the whole text does. */
  words: string | null;
}

/**
 * Fills the room the other blocks of `box`'s parent leave it, the parent
 * standing in a column: its picture (`pictureIn`) as tall as the room allows
 * with the whole of `text` under it, between `widest` and `tallest` (width
 * over height); and when the whole text does not fit even under the lowest
 * picture, the words of it that do. Measured again whenever the room
 * changes, on an unseen copy of the box, the page's own being React's to
 * draw. Null when `enabled` is off.
 */
export function useFilledRoom(
  box: React.RefObject<HTMLElement | null>,
  text: string | null,
  wordsIn: string,
  pictureIn: string,
  widest: number,
  tallest: number,
  enabled: boolean,
): FilledRoom | null {
  const [filled, setFilled] = useState<FilledRoom | null>(null);
  useLayoutEffect(() => {
    const own = box.current;
    const column = own?.parentElement;
    if (!enabled || !own || !column) {
      setFilled(null);
      return;
    }
    const fill = () => {
      const others = [...column.children].filter((child) => child !== own);
      const gap = parseFloat(getComputedStyle(column).rowGap) || 0;
      const room =
        column.clientHeight -
        others.reduce((taken, other) => taken + (other as HTMLElement).offsetHeight, 0) -
        gap * others.length;
      const copy = own.cloneNode(true) as HTMLElement;
      copy.dataset.measuring = "";
      const words = copy.querySelector(wordsIn);
      const picture = copy.querySelector<HTMLElement>(pictureIn);
      Object.assign(copy.style, {
        position: "absolute",
        visibility: "hidden",
        width: `${own.clientWidth}px`,
        height: "auto",
      });
      own.after(copy);
      const across = picture?.offsetWidth ?? 0;
      const lowest = across / widest;
      const highest = across / tallest;
      const heightWith = (pictureHeight: number, shown: string | null) => {
        if (picture) {
          Object.assign(picture.style, pictureHeld(pictureHeight));
        }
        if (words && shown !== null) {
          words.textContent = shown;
        }
        return copy.offsetHeight;
      };
      const spare = room - heightWith(lowest, text);
      let found: FilledRoom;
      if (spare >= -SLACK) {
        found = { pictureHeight: Math.min(lowest + Math.max(0, spare), highest), words: null };
      } else {
        const kept =
          text && words ? wordsThatFit(text, (candidate) => heightWith(lowest, candidate) <= room + SLACK) : null;
        // A screen too short for even a line of it under the lowest picture
        // takes what is missing off the picture.
        const over = heightWith(lowest, kept ?? "") - room;
        found = { pictureHeight: Math.max(0, lowest - Math.max(0, over)), words: kept };
      }
      copy.remove();
      setFilled((was) =>
        was && Math.abs(was.pictureHeight - found.pictureHeight) < 1 && was.words === found.words ? was : found,
      );
    };
    /* The room changes with the column and with every other block in it,
       some of which come in after the box does: the lists of tracks once
       the device has said what it plays. The unseen copy coming and going
       is no change at all. */
    const sizes = new ResizeObserver(fill);
    const watch = () => {
      sizes.disconnect();
      sizes.observe(column);
      for (const child of column.children) {
        if (child !== own) {
          sizes.observe(child);
        }
      }
    };
    const blocks = new MutationObserver((changes) => {
      const isTheCopy = (node: Node) => node instanceof HTMLElement && "measuring" in node.dataset;
      if (changes.some((change) => ![...change.addedNodes, ...change.removedNodes].every(isTheCopy))) {
        watch();
      }
    });
    watch();
    blocks.observe(column, { childList: true });
    // The words take their true room only once their face has come.
    document.fonts.addEventListener("loadingdone", fill);
    return () => {
      sizes.disconnect();
      blocks.disconnect();
      document.fonts.removeEventListener("loadingdone", fill);
    };
  }, [box, text, wordsIn, pictureIn, widest, tallest, enabled]);
  return filled;
}

/** How a picture is drawn at this height, its own shape set aside and the
 *  picture cut to fill it. */
export function pictureHeld(height: number): { height: string; aspectRatio: string } {
  return { height: `${height}px`, aspectRatio: "auto" };
}
