/*
 * The first seconds of a film, picture by picture.
 *
 * A film that stutters once as it starts and plays smoothly afterwards is
 * invisible to everything else the page follows: the clock never stops for a
 * whole second, and a picture or two held a moment too long is not a picture
 * the browser counts as dropped. Only the time each picture reached the
 * screen says it, and it is read here for the opening seconds of every film.
 *
 * Three different faults look the same from a chair. A picture held on the
 * screen too long: nothing new was ready in time. Pictures of the film never
 * shown: they were ready and thrown away, and the film jumps. And the page
 * itself too busy to look, when the browser went on showing pictures without
 * telling this page about each one: not a fault on screen at all, and told
 * apart so it is never read as one.
 */

import type { HitchKind, OpeningSeconds } from "../api";

/** One picture as the browser says it reached the screen. */
export interface Presented {
  /** When it was shown, on the page's clock, in milliseconds. */
  shownAt: number;
  /** How many pictures the browser has handed to the screen so far. */
  presented: number;
  /** The moment of the film on it, in seconds. */
  media: number;
  /** Set when the film was paused or moved just before: the time since the
   *  picture before it says nothing about how smoothly it played. */
  afterABreak: boolean;
}

/** Where the film's own clock stood at one moment on the page's clock. It
 *  tells a picture held while the clock ran on from the clock itself having
 *  stopped, which only the element can say, since no picture is shown then. */
export interface ClockReading {
  at: number;
  /** The film's clock, in seconds. */
  clock: number;
}

/** A stretch when the page's own work kept it from doing anything else. */
export interface BusySpell {
  startedAt: number;
  lasted: number;
}

/** What the opening seconds came to, before what only the element knows. */
export type Opening = Omit<
  OpeningSeconds,
  | "pictures_dropped"
  | "waited"
  | "busy_spells"
  | "busy_ms"
  | "films_before_in_this_tab"
  | "page_age_ms"
  | "sound_on"
>;

/** How much longer than one picture a gap has to last to be a hitch. Half a
 *  picture more covers the uneven cadence of a film of twenty four pictures
 *  a second on a screen of sixty, which alternates two and three refreshes. */
const HITCH_OVER = 1.5;
/** What a picture lasts when the film says nothing usable about it. */
const A_PICTURE_AT_SIXTY_MS = 1000 / 60;
/** How many hitches are named one by one. The rest are counted. */
export const HITCHES_NAMED = 8;

function median(values: number[]): number | null {
  if (values.length === 0) {
    return null;
  }
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
}

function busyDuring(spells: BusySpell[], from: number, to: number): number {
  let busy = 0;
  for (const spell of spells) {
    const overlap = Math.min(to, spell.startedAt + spell.lasted) - Math.max(from, spell.startedAt);
    if (overlap > 0) {
      busy += overlap;
    }
  }
  return busy;
}

/** How far the film's clock went between two moments, in milliseconds, read
 *  from the last reading at or before the first and the first at or after
 *  the second. Nothing when the readings do not reach that far. */
function clockDuring(readings: ClockReading[], from: number, to: number): number | null {
  let before: ClockReading | undefined;
  let after: ClockReading | undefined;
  for (const reading of readings) {
    if (reading.at <= from) {
      before = reading;
    }
    if (reading.at >= to && after === undefined) {
      after = reading;
    }
  }
  return before && after ? Math.round((after.clock - before.clock) * 1000) : null;
}

/** Reads the pictures of the opening seconds into what went wrong in them. */
export function readTheOpening(
  pictures: Presented[],
  busy: BusySpell[],
  clock: ClockReading[],
): Opening {
  const steps: number[] = [];
  for (let index = 1; index < pictures.length; index += 1) {
    const before = pictures[index - 1];
    const after = pictures[index];
    const media = (after.media - before.media) * 1000;
    if (!after.afterABreak && after.presented - before.presented === 1 && media > 0) {
      steps.push(media);
    }
  }
  const pictureMs = median(steps) ?? A_PICTURE_AT_SIXTY_MS;
  const limit = pictureMs * HITCH_OVER;

  const first = pictures[0]?.shownAt ?? 0;
  const opening: Opening = {
    over_ms: pictures.length > 0 ? Math.round(pictures[pictures.length - 1].shownAt - first) : 0,
    pictures: pictures.length,
    picture_ms: Math.round(pictureMs * 10) / 10,
    held: 0,
    worst_held_ms: 0,
    skipped: 0,
    pictures_lost: 0,
    blind: 0,
    worst_blind_ms: 0,
    first_hitches: [],
  };

  for (let index = 1; index < pictures.length; index += 1) {
    const before = pictures[index - 1];
    const after = pictures[index];
    if (after.afterABreak) {
      continue;
    }
    const gap = after.shownAt - before.shownAt;
    const media = (after.media - before.media) * 1000;
    let kind: HitchKind | null = null;
    let lost = 0;
    if (after.presented - before.presented > 1) {
      if (gap > limit) {
        kind = "blind";
        opening.blind += 1;
        opening.worst_blind_ms = Math.max(opening.worst_blind_ms, Math.round(gap));
      }
    } else if (media > limit) {
      kind = "skipped";
      lost = Math.round(media / pictureMs) - 1;
      opening.skipped += 1;
      opening.pictures_lost += lost;
    } else if (gap > limit) {
      kind = "held";
      opening.held += 1;
      opening.worst_held_ms = Math.max(opening.worst_held_ms, Math.round(gap));
    }
    if (kind !== null && opening.first_hitches.length < HITCHES_NAMED) {
      opening.first_hitches.push({
        kind,
        at_ms: Math.round(before.shownAt - first),
        at_second: Math.round(after.media * 1000) / 1000,
        gap_ms: Math.round(gap),
        pictures_lost: lost,
        busy_ms: Math.round(busyDuring(busy, before.shownAt, after.shownAt)),
        clock_ms: clockDuring(clock, before.shownAt, after.shownAt),
      });
    }
  }
  return opening;
}

/** How long the opening is followed, from its first picture. */
const OPENING_MS = 15_000;

/** Films this tab started before the one being followed: the first of them is
 *  the one started cold. */
let filmsBefore = 0;

function droppedSoFar(element: HTMLVideoElement): number {
  return element.getVideoPlaybackQuality?.().droppedVideoFrames ?? 0;
}

/**
 * Follows the opening seconds of one film and says what they came to, once:
 * when they are over, or when the film is left before that. Hands back what
 * stops it.
 */
export function followTheOpening(
  element: HTMLVideoElement,
  say: (opening: OpeningSeconds) => void,
): () => void {
  const filmsBeforeThis = filmsBefore;
  filmsBefore += 1;
  if (!element.requestVideoFrameCallback) {
    return () => {};
  }

  const pictures: Presented[] = [];
  const busy: BusySpell[] = [];
  const clock: ClockReading[] = [];
  let soundOn = false;
  let reading = 0;
  let broken = false;
  let waited = 0;
  let droppedAtFirst = 0;
  let pageAge = 0;
  let pending: number | null = null;
  let said = false;

  const aBreak = () => {
    broken = true;
  };
  const waiting = () => {
    if (pictures.length > 0) {
      waited += 1;
    }
  };
  element.addEventListener("pause", aBreak);
  element.addEventListener("seeking", aBreak);
  element.addEventListener("waiting", waiting);

  const watchingBusy = PerformanceObserver.supportedEntryTypes?.includes("longtask") ?? false;
  const busyWatcher = watchingBusy
    ? new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
          busy.push({ startedAt: entry.startTime, lasted: entry.duration });
        }
      })
    : null;
  busyWatcher?.observe({ type: "longtask" });

  const finish = () => {
    if (pending !== null) {
      element.cancelVideoFrameCallback?.(pending);
      pending = null;
    }
    cancelAnimationFrame(reading);
    busyWatcher?.disconnect();
    element.removeEventListener("pause", aBreak);
    element.removeEventListener("seeking", aBreak);
    element.removeEventListener("waiting", waiting);
    if (said || pictures.length < 2) {
      return;
    }
    said = true;
    const from = pictures[0].shownAt;
    const to = pictures[pictures.length - 1].shownAt;
    const inside = busy.filter((spell) => spell.startedAt + spell.lasted > from && spell.startedAt < to);
    say({
      ...readTheOpening(pictures, busy, clock),
      pictures_dropped: droppedSoFar(element) - droppedAtFirst,
      waited,
      busy_spells: watchingBusy ? inside.length : null,
      busy_ms: watchingBusy ? Math.round(inside.reduce((sum, spell) => sum + spell.lasted, 0)) : null,
      films_before_in_this_tab: filmsBeforeThis,
      page_age_ms: pageAge,
      sound_on: soundOn,
    });
  };

  const onPicture = (now: number, picture: VideoFrameCallbackMetadata) => {
    pending = null;
    const shownAt = picture.expectedDisplayTime || now;
    if (pictures.length === 0) {
      droppedAtFirst = droppedSoFar(element);
      pageAge = Math.round(shownAt);
      soundOn = !element.muted && element.volume > 0;
      broken = false;
    }
    pictures.push({
      shownAt,
      presented: picture.presentedFrames,
      media: picture.mediaTime,
      afterABreak: broken,
    });
    broken = false;
    if (shownAt - pictures[0].shownAt >= OPENING_MS) {
      finish();
      return;
    }
    pending = element.requestVideoFrameCallback(onPicture);
  };
  pending = element.requestVideoFrameCallback(onPicture);

  const readTheClock = () => {
    clock.push({ at: performance.now(), clock: element.currentTime });
    reading = requestAnimationFrame(readTheClock);
  };
  reading = requestAnimationFrame(readTheClock);

  return finish;
}
