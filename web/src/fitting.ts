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

/**
 * The words of `text` that fit in `box`, a box given all the room it may take
 * and no more, cut again whenever that room changes. The cutting is measured
 * on an unseen copy of the box, the page's own being React's to draw. Null
 * when the whole text fits, or when `enabled` is off.
 */
export function useWordsThatFit(
  box: React.RefObject<HTMLElement | null>,
  text: string | null,
  wordsIn: string,
  enabled: boolean,
): string | null {
  const [shown, setShown] = useState<string | null>(null);
  useLayoutEffect(() => {
    const room = box.current;
    if (!enabled || !text || !room) {
      setShown(null);
      return;
    }
    const cut = () => {
      const copy = room.cloneNode(true) as HTMLElement;
      const words = copy.querySelector(wordsIn);
      if (!words) {
        return;
      }
      Object.assign(copy.style, {
        position: "absolute",
        visibility: "hidden",
        width: `${room.clientWidth}px`,
        height: `${room.clientHeight}px`,
        flex: "none",
      });
      room.after(copy);
      const fitted = wordsThatFit(text, (candidate) => {
        words.textContent = candidate;
        return copy.scrollHeight <= copy.clientHeight + SLACK;
      });
      copy.remove();
      setShown(fitted === text ? null : fitted);
    };
    const watcher = new ResizeObserver(cut);
    watcher.observe(room);
    return () => watcher.disconnect();
  }, [box, text, wordsIn, enabled]);
  return shown;
}
