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

/** One way a box can be laid out: a class it wears, and how wide its
 *  picture can be that way, as a share of the box. */
export interface Way {
  className: string | null;
  least: number;
  most: number;
}

/** How a box standing in a column fills the room the others leave it. */
export interface FilledRoom {
  /** Which of the ways it is laid out. */
  way: number;
  /** How wide its picture is, as a share of the box. */
  share: number;
  /** The words of its text that fit, or nothing when the whole text does. */
  words: string | null;
}

/**
 * Fills the room the other blocks of `box`'s parent leave it, the parent
 * standing in a column: the first of `ways` the whole of `text` fits in,
 * with its picture (`pictureIn`) as wide as that way lets it be and the
 * room still allows; and when the whole text fits no way at all, the last
 * way with the smallest picture and the words of the text that do fit.
 * Measured again whenever the room changes, on an unseen copy of the box,
 * the page's own being React's to draw. Null when `enabled` is off.
 */
export function useFilledRoom(
  box: React.RefObject<HTMLElement | null>,
  text: string | null,
  wordsIn: string,
  pictureIn: string,
  ways: readonly Way[],
  enabled: boolean,
): FilledRoom | null {
  const [filled, setFilled] = useState<FilledRoom | null>(null);
  useLayoutEffect(() => {
    const own = box.current;
    const column = own?.parentElement;
    if (!enabled || !own || !column || ways.length === 0) {
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
      const fits = (way: Way, share: number, shown: string | null) => {
        for (const other of ways) {
          if (other.className) {
            copy.classList.toggle(other.className, other === way);
          }
        }
        if (picture) {
          Object.assign(picture.style, pictureShare(share));
        }
        if (words && shown !== null) {
          words.textContent = shown;
        }
        return copy.offsetHeight <= room + SLACK;
      };
      let found: FilledRoom | null = null;
      for (const [at, way] of ways.entries()) {
        if (!fits(way, way.least, text)) {
          continue;
        }
        let fitting = way.least;
        let tooWide = way.most + 1;
        while (tooWide - fitting > 1) {
          const middle = Math.floor((fitting + tooWide) / 2);
          if (fits(way, middle, text)) {
            fitting = middle;
          } else {
            tooWide = middle;
          }
        }
        found = { way: at, share: fitting, words: null };
        break;
      }
      if (!found) {
        const at = ways.length - 1;
        const last = ways[at];
        found = {
          way: at,
          share: last.least,
          words: text && words ? wordsThatFit(text, (candidate) => fits(last, last.least, candidate)) : null,
        };
      }
      copy.remove();
      const settled = found;
      setFilled((was) =>
        was?.way === settled.way && was.share === settled.share && was.words === settled.words ? was : settled,
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
  }, [box, text, wordsIn, pictureIn, ways, enabled]);
  return filled;
}

/** The share of a box a picture takes when it takes all of it. */
export const WHOLE_WIDTH = 100;

/** How a picture is drawn at this share of its box: across the whole of it,
 *  with no room kept beside it for words that go under it instead. */
export function pictureShare(share: number): { width: string; marginRight: string } {
  return share === WHOLE_WIDTH ? { width: "100%", marginRight: "0" } : { width: `${share}%`, marginRight: "" };
}
