import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";

/*
 * How far there is left to wait, told as one number.
 *
 * Built from real moments rather than guessed at. Every one of the stages
 * below is something that genuinely happens once, in this order, on the way
 * to a film playing: a session is asked for, hls.js reads the playlist, the
 * server produces what the very first piece needs, that piece reaches this
 * browser, and the browser reads enough of it to know it has a film. Where two
 * of those are seconds apart the number climbs quickly; where one of them is
 * a slow link labouring over a few megabytes it climbs slowly and keeps
 * climbing, because it is still waiting on that one real thing and says so
 * rather than promising an end it cannot see.
 *
 * Between one stage and the next, nothing new is known yet, so the number
 * creeps toward the next stage's floor rather than sitting still: reaching
 * that floor exactly is what waits for the real event, and the creep only
 * ever approaches it, never reaches or passes it uninvited. A stage that
 * turns out to take far longer than usual still only ever creeps toward the
 * same ceiling, however long it takes to get there.
 *
 * Every step is written to Melyxar's own journal, under the `page` tag, with
 * how long the stage before it took: the one thing this cannot know on its
 * own is whether a minute on a slow link is normal or a sign that something
 * is actually stuck, and reading that needs a real connection to try it on,
 * not a guess made here. A console open on the machine watching the film is
 * not something to rely on; the journal is read from wherever the server is.
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

/** Where the number sits the instant each stage is reached. */
const LOADING_FLOOR: Record<LoadingStage, number> = {
  opening: 0,
  // A session is asked for.
  session_opened: 6,
  // hls.js has read the playlist and knows what to ask for next.
  manifest_parsed: 14,
  // The server has said it is actively writing the piece this browser needs.
  producing: 24,
  // That piece exists on the server now; what is left is getting it here.
  produced: 58,
  // It has arrived and been read. What is left is the browser noticing.
  first_fragment_loaded: 90,
  done: 100,
};

/**
 * How many milliseconds a stage is expected to take, roughly, before the
 * number climbing through it starts to visibly slow down.
 *
 * A first guess rather than a measurement, and said so where it is used: the
 * two that matter most, `producing` and `produced`, are exactly the two a
 * slow link or a slow disk stretches furthest, and are the two worth
 * correcting first from what the console actually shows.
 */
const LOADING_TAU_MS: Record<Exclude<LoadingStage, "done">, number> = {
  opening: 400,
  session_opened: 1200,
  manifest_parsed: 900,
  producing: 3500,
  produced: 3200,
  first_fragment_loaded: 350,
};

/** A line in Melyxar's own journal, when there is a session to write it
 *  against: before one exists there is nothing yet worth a line. Written only
 *  at the server's own debug level, the same as every other fact a page tells
 *  the journal, so it says nothing at all unless it is asked to. */
function tellTheJournalOfAStage(session: string | null, stage: LoadingStage, afterMs: number): void {
  if (!session) {
    return;
  }
  api
    .tellTheJournal({ session, saw: "loading_stage", stage, after_ms: Math.round(afterMs) })
    .catch(() => {
      // A line that did not arrive is a line the journal never had to begin
      // with: nothing here is worth troubling a viewer over.
    });
}

/** Whether a stage comes after another on the way to a playing film. */
export function isLater(stage: LoadingStage, than: LoadingStage): boolean {
  return LOADING_STAGES.indexOf(stage) > LOADING_STAGES.indexOf(than);
}

/**
 * The number shown after `elapsedMs` in a stage: from the stage's floor
 * toward the next one's, approaching it and never quite reaching it, since
 * reaching it is what the next real stage is for, not a clock running out.
 */
export function percentAt(stage: LoadingStage, elapsedMs: number): number {
  if (stage === "done") {
    return LOADING_FLOOR.done;
  }
  const next = LOADING_STAGES[LOADING_STAGES.indexOf(stage) + 1];
  const floor = LOADING_FLOOR[stage];
  const ceiling = LOADING_FLOOR[next];
  return ceiling - (ceiling - floor) * Math.exp(-elapsedMs / LOADING_TAU_MS[stage]);
}

/**
 * The number itself, where it stands, and the two ways it is moved: started
 * over for a session opened from nothing, and carried on to a later stage.
 *
 * `stream` is the session being waited on, and the number runs for as long
 * as the picture it will show, `pictureKey`, is not yet the one ready.
 */
export function useLoading(
  stream: string | null,
  readyPicture: string | null,
  pictureKey: string | null,
) {
  /** How far there is left to wait, shown while the picture is not there. */
  const [loadingPercent, setLoadingPercent] = useState(0);
  /* Which of the real moments on the way to a playing film has been reached,
     and when, in the browser's own clock rather than the wall clock: what
     matters is how long a stage has been sat in, not what time it is. */
  const loadingStage = useRef<LoadingStage>("opening");
  const loadingStageSince = useRef(0);

  /* Starts over from nothing, for a session opened from nothing: a viewer
     changing quality mid film is not owed the tail end of the last session's
     progress carried into this one. */
  const resetLoadingStage = useCallback(() => {
    loadingStage.current = "opening";
    loadingStageSince.current = performance.now();
    setLoadingPercent(LOADING_FLOOR.opening);
  }, []);

  /* Moved on to a later stage, and only ever a later one: a message from a
     session already left behind, or one that arrived after a further one,
     must not walk the number backwards.

     Told to the journal against the session this is happening for, when
     there is one to name: a viewer with a slow connection can be asked to
     copy a line out of a console, but the journal is there whether or not
     anybody thought to open one before the film started. */
  const enterLoadingStage = useCallback((session: string | null, stage: LoadingStage) => {
    if (!isLater(stage, loadingStage.current)) {
      return;
    }
    const now = performance.now();
    tellTheJournalOfAStage(session, stage, now - loadingStageSince.current);
    loadingStage.current = stage;
    loadingStageSince.current = now;
    setLoadingPercent(LOADING_FLOOR[stage]);
  }, []);

  /* The number itself, moved along between one real stage and the next.
     Ticked on a plain timer rather than redrawn only when a stage changes,
     because a number that only ever jumps between six fixed points does not
     read as something happening; a number that keeps creeping does, which is
     the one thing a viewer watching it is actually asking it for. Run for as
     long as the notice above it is shown and not a moment longer. */
  useEffect(() => {
    if (!stream || readyPicture === pictureKey) {
      return;
    }
    const tick = () => {
      const stage = loadingStage.current;
      if (stage === "done") {
        return;
      }
      setLoadingPercent(percentAt(stage, performance.now() - loadingStageSince.current));
    };
    tick();
    const timer = window.setInterval(tick, 120);
    return () => window.clearInterval(timer);
  }, [stream, readyPicture, pictureKey]);

  return { loadingPercent, resetLoadingStage, enterLoadingStage };
}
