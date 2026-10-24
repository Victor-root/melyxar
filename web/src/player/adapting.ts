/*
 * The automatic quality at work on one film: measuring the connection before
 * it, following it while it plays, and moving the rung when it has to.
 *
 * What to do is decided in adaptive.ts; this only gathers what it needs, the
 * speed of each segment that arrives and every time the film stops to wait,
 * and writes each move into the journal.
 */

import { useCallback, useEffect, useRef, useState } from "react";

import { api } from "../api";
import { ADAPTIVE_RUNGS, UNMEASURED_RUNG, afterMoving, believed, nextMove, rungFor, startingOn, withMeasure } from "./adaptive";
import type { Course, Film, Speed } from "./adaptive";
import { connectionSpeed } from "./connection";
import type { Picture } from "./picture";
import { ADAPTIVE } from "./quality";
import type { Quality } from "./quality";
import { whatIsHeldAround } from "./watch";

/** A segment smaller than this arrives too fast to be timed fairly. */
const TIMED_FROM_BYTES = 64 * 1024;
/** A wait this soon after the film opened or jumped is the film arriving,
 *  not the connection failing it. */
const OPENING_MS = 5_000;
const AFTER_A_JUMP_MS = 4_000;

/** A rate in kilobits a second, nothing for the film as it is. */
const kbps = (bitrate: number | null) => (bitrate === null ? undefined : Math.round(bitrate / 1000));

export interface Adapting {
  /** The rung to ask the server for: the viewer's own, or the one chosen for
   *  the connection. Null while the connection is being measured. */
  asked: Quality | null;
  /** The rung the automatic quality stands on, when it is the one chosen. */
  rung: Quality | null;
  /** Told of each segment that arrived: its size, and how long it took from
   *  its first byte to its last. */
  heardSegment: (bytes: number, flowingMs: number) => void;
}

export function useAdaptiveQuality({
  quality,
  film,
  video,
  readyPicture,
  sourceId,
}: {
  quality: Quality;
  /** What the film is written at, once the server has said. */
  film: Film | null;
  video: React.RefObject<Picture | null>;
  /** Changes with every picture opened, which is when the stalls of a new
   *  one start being listened to. */
  readyPicture: string | null;
  sourceId: string;
}): Adapting {
  const on = quality.key === ADAPTIVE.key;
  const [rung, setRung] = useState<number | null>(null);
  const course = useRef<Course | null>(null);
  const speed = useRef<Speed | null>(null);
  const filmHeld = useRef(film);
  filmHeld.current = film;

  const tell = useCallback(
    (why: "measured" | "stalls" | "slower_link" | "faster_link", from: number | null, to: number) => {
      api
        .tellTheJournal({
          source: sourceId,
          saw: "quality_adapted",
          why,
          measured_kbps: speed.current ? kbps(believed(speed.current)) : undefined,
          from_kbps: from === null ? undefined : kbps(ADAPTIVE_RUNGS[from].bitrate),
          to_kbps: kbps(ADAPTIVE_RUNGS[to].bitrate),
          to_height: ADAPTIVE_RUNGS[to].height ?? undefined,
        })
        .catch(() => {});
    },
    [sourceId],
  );

  /* Measured once when the automatic quality is chosen, before the film is
     asked for: asking first and correcting after would open it twice. */
  useEffect(() => {
    course.current = null;
    setRung(null);
    if (!on) {
      return;
    }
    let gone = false;
    void connectionSpeed().then((bitsPerSecond) => {
      if (gone) {
        return;
      }
      speed.current = bitsPerSecond === null ? null : withMeasure(null, bitsPerSecond, 0);
      const first = bitsPerSecond === null ? UNMEASURED_RUNG : rungFor(bitsPerSecond);
      course.current = startingOn(first, performance.now());
      setRung(first);
      tell("measured", null, first);
    });
    return () => {
      gone = true;
    };
  }, [on, tell]);

  const consider = useCallback(() => {
    const now = performance.now();
    const element = video.current;
    const current = course.current;
    if (!current || !element) {
      return;
    }
    const heldTo = whatIsHeldAround(element, element.currentTime).held_to_second;
    const heldAhead = heldTo === null ? 0 : heldTo - element.currentTime;
    const move = nextMove(current, speed.current ? believed(speed.current) : null, heldAhead, now, filmHeld.current);
    if (move) {
      course.current = afterMoving(current, move, now);
      setRung(move.rung);
      tell(move.because, current.rung, move.rung);
    }
  }, [video, tell]);

  const heardSegment = useCallback(
    (bytes: number, flowingMs: number) => {
      const current = course.current;
      if (!current || bytes < TIMED_FROM_BYTES || flowingMs <= 0) {
        return;
      }
      const seconds = flowingMs / 1000;
      speed.current = withMeasure(speed.current, (bytes * 8) / seconds, seconds);
      course.current = { ...current, measured: current.measured + 1 };
      consider();
    },
    [consider],
  );

  /* Every time the film stops to wait for what has not arrived, other than
     while it opens or right after a jump. */
  useEffect(() => {
    const element = video.current;
    if (!on || !element || readyPicture === null) {
      return;
    }
    let startedAt: number | null = null;
    let jumpedAt = 0;
    const started = () => {
      startedAt ??= performance.now();
    };
    const jumped = () => {
      jumpedAt = performance.now();
    };
    const waited = () => {
      const now = performance.now();
      const current = course.current;
      if (
        !current ||
        element.seeking ||
        element.paused ||
        startedAt === null ||
        now - startedAt < OPENING_MS ||
        now - jumpedAt < AFTER_A_JUMP_MS
      ) {
        return;
      }
      course.current = { ...current, stalls: [...current.stalls, now] };
      consider();
    };
    element.addEventListener("playing", started);
    element.addEventListener("seeking", jumped);
    element.addEventListener("waiting", waited);
    return () => {
      element.removeEventListener("playing", started);
      element.removeEventListener("seeking", jumped);
      element.removeEventListener("waiting", waited);
    };
  }, [on, readyPicture, video, consider]);

  const standingOn = on && rung !== null ? ADAPTIVE_RUNGS[rung] : null;
  return { asked: on ? standingOn : quality, rung: standingOn, heardSegment };
}
