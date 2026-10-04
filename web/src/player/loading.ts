import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";

/*
 * How far there is left to wait, told as one number, and only ever from what
 * has really been measured: nothing here is a clock guessing.
 *
 * Read on a real film, the wait is almost entirely one thing: the server
 * writing the first piece of film the browser needs, then that piece crossing
 * the network. The browser shows the picture the instant it has read it. Both
 * are measured as they happen: the server says how much of that piece is
 * written, from what the tool producing it reports, and the browser counts
 * the bytes of it arriving. The few moments around them (a session opened,
 * the playlist read) are real events that each move the number a little.
 *
 * The number shown glides toward what was measured, so a reading every
 * quarter of a second reads as a steady climb, and never goes past it or back.
 *
 * Every stage is written to Melyxar's own journal, under the `page` tag, with
 * how long the stage before it took: that is what tells a slow link from
 * something actually stuck, read from wherever the server is.
 */
export type LoadingStage =
  | "opening"
  | "session_opened"
  | "manifest_parsed"
  | "producing"
  | "produced"
  | "first_fragment_loaded"
  | "done";

const LOADING_STAGES: LoadingStage[] = [
  "opening",
  "session_opened",
  "manifest_parsed",
  "producing",
  "produced",
  "first_fragment_loaded",
  "done",
];

/** The stretch the server writing the first piece fills. */
const WRITING_FROM = 8;
/** The stretch that piece crossing the network fills, up to the browser
 *  having it whole. */
const CARRYING_FROM = 85;
const CARRYING_TO = 98;

/** Where each stage puts the number, at the least. */
const LOADING_FLOOR: Record<LoadingStage, number> = {
  opening: 0,
  session_opened: 3,
  manifest_parsed: 6,
  producing: WRITING_FROM,
  produced: CARRYING_FROM,
  first_fragment_loaded: CARRYING_TO,
  done: 100,
};

/** What is known of the wait at one instant. */
export interface Measured {
  stage: LoadingStage;
  /** How much of the first piece the server has written, from 0 to 1. */
  written: number;
  /** How much of it has reached this browser, from 0 to 1. */
  carried: number;
}

/** The number all of that is worth. */
export function percentOf({ stage, written, carried }: Measured): number {
  const share = (from: number, to: number, part: number) =>
    from + (to - from) * Math.min(1, Math.max(0, part));
  return Math.max(
    LOADING_FLOOR[stage],
    written > 0 ? share(WRITING_FROM, CARRYING_FROM, written) : 0,
    written >= 1 || carried > 0 ? share(CARRYING_FROM, CARRYING_TO, carried) : 0,
  );
}

/** One step of the number shown toward what was measured. */
export function glideToward(shown: number, measured: number): number {
  if (measured <= shown) {
    return shown;
  }
  const next = shown + (measured - shown) * GLIDE;
  return measured - next < 0.2 ? measured : next;
}

/** How much of the way to what was measured one step covers. */
const GLIDE = 0.3;

/** How often the number shown takes a step. */
const STEP_EVERY = 80;

/** A line in Melyxar's own journal, when there is a session to write it
 *  against. Written only at the server's own debug level. */
function tellTheJournalOfAStage(session: string | null, stage: LoadingStage, afterMs: number): void {
  if (!session) {
    return;
  }
  api
    .tellTheJournal({ session, saw: "loading_stage", stage, after_ms: Math.round(afterMs) })
    .catch(() => {
      // A line that did not arrive is not worth troubling a viewer over.
    });
}

/** Whether a stage comes after another on the way to a playing film. */
export function isLater(stage: LoadingStage, than: LoadingStage): boolean {
  return LOADING_STAGES.indexOf(stage) > LOADING_STAGES.indexOf(than);
}

/**
 * The number itself, and what moves it: started over for a session opened
 * from nothing, a later stage reached, the server's account of the first
 * piece, and that piece arriving.
 *
 * `stream` is the session being waited on, and the number runs for as long
 * as the picture it will show, `pictureKey`, is not yet the one ready.
 */
export function useLoading(
  stream: string | null,
  readyPicture: string | null,
  pictureKey: string | null,
) {
  const [loadingPercent, setLoadingPercent] = useState(0);
  const measured = useRef<Measured>({ stage: "opening", written: 0, carried: 0 });
  /* When the current stage began, in the browser's own clock: what the
     journal is told is how long a stage was sat in. */
  const stageSince = useRef(0);
  /* Reads how much of the first piece has arrived, while it is arriving. */
  const carriedNow = useRef<(() => number) | null>(null);

  const resetLoadingStage = useCallback(() => {
    measured.current = { stage: "opening", written: 0, carried: 0 };
    carriedNow.current = null;
    stageSince.current = performance.now();
    setLoadingPercent(0);
  }, []);

  /* Only ever a later stage: a message from a session already left behind
     must not walk the number backwards. */
  const enterLoadingStage = useCallback((session: string | null, stage: LoadingStage) => {
    if (!isLater(stage, measured.current.stage)) {
      return;
    }
    const now = performance.now();
    tellTheJournalOfAStage(session, stage, now - stageSince.current);
    measured.current = { ...measured.current, stage };
    stageSince.current = now;
    if (stage === "done") {
      setLoadingPercent(100);
    }
  }, []);

  /* The server's account of the first piece. Never less than before: a tool
     set going again counts its piece from nothing, and the viewer has not
     lost what was already waited through. */
  const noteWritten = useCallback((written: number) => {
    measured.current = {
      ...measured.current,
      written: Math.max(measured.current.written, written),
    };
  }, []);

  /* Handed a way to read the first piece arriving, once it starts to. */
  const watchCarrying = useCallback((read: () => number) => {
    carriedNow.current = read;
  }, []);

  useEffect(() => {
    if (!stream || readyPicture === pictureKey) {
      return;
    }
    const step = () => {
      const now = measured.current;
      if (now.stage === "done") {
        return;
      }
      const carried = carriedNow.current?.() ?? 0;
      measured.current = { ...now, carried: Math.max(now.carried, carried) };
      const target = percentOf(measured.current);
      setLoadingPercent((shown) => glideToward(shown, target));
    };
    step();
    const timer = window.setInterval(step, STEP_EVERY);
    return () => window.clearInterval(timer);
  }, [stream, readyPicture, pictureKey]);

  return { loadingPercent, resetLoadingStage, enterLoadingStage, noteWritten, watchCarrying };
}
