/*
 * What a recording of the page's smoothness says, worked out and written.
 *
 * Kept apart from the recorder, which only reads the browser, so that the
 * sums can be checked without one: where a pass of scrolling begins and ends,
 * how many frames a stall cost, what a frame's worth of time even is on the
 * screen that recorded it.
 */

/** Where the page stood at one moment. */
export interface Scrolled {
  at: number;
  top: number;
}

/** One stretch of scrolling in one direction. */
export interface Pass {
  direction: "down" | "up";
  from: number;
  to: number;
  startTop: number;
  endTop: number;
}

/** A move smaller than this is a hand settling, not a pass. */
const A_REAL_MOVE = 80;

/** A stillness this long ends a pass even without a turn. */
const A_PAUSE_MS = 1500;

/**
 * The passes a list of positions makes: each ends where the page turns back
 * by more than a hand settling, or where it stood still a while.
 */
export function passesOf(samples: Scrolled[]): Pass[] {
  const passes: Pass[] = [];
  let start: Scrolled | null = null;
  let far: Scrolled | null = null;
  let direction: Pass["direction"] | null = null;
  let last: Scrolled | null = null;

  const close = () => {
    if (start && far && direction && Math.abs(far.top - start.top) >= A_REAL_MOVE) {
      passes.push({ direction, from: start.at, to: far.at, startTop: start.top, endTop: far.top });
    }
  };

  for (const sample of samples) {
    if (!start || !far || (last && sample.at - last.at > A_PAUSE_MS)) {
      close();
      start = last && sample.at - last.at <= A_PAUSE_MS ? last : sample;
      far = sample;
      direction = null;
      last = sample;
      continue;
    }
    if (direction === null) {
      if (Math.abs(sample.top - start.top) >= A_REAL_MOVE) {
        direction = sample.top > start.top ? "down" : "up";
      }
      far = sample;
    } else {
      const further = direction === "down" ? sample.top >= far.top : sample.top <= far.top;
      if (further) {
        far = sample;
      } else if (Math.abs(sample.top - far.top) >= A_REAL_MOVE) {
        close();
        start = far;
        direction = sample.top > far.top ? "down" : "up";
        far = sample;
      }
    }
    last = sample;
  }
  close();
  return passes;
}

/** The value below which a share of the values lie. */
export function percentile(values: number[], share: number): number {
  if (values.length === 0) {
    return 0;
  }
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(share * sorted.length) - 1));
  return sorted[index];
}

/** How many frames a gap between two drawn frames let go by. */
export function framesLost(gap: number, frame: number): number {
  return frame > 0 ? Math.max(0, Math.round(gap / frame) - 1) : 0;
}

export interface FrameStats {
  frames: number;
  lost: number;
  /** Gaps of more than one and a half frames. */
  stalls: number;
  p50: number;
  p95: number;
  worst: number;
}

/** What a list of gaps between frames says, on a screen whose frame lasts
 *  this long. */
export function frameStats(gaps: number[], frame: number): FrameStats {
  return {
    frames: gaps.length,
    lost: gaps.reduce((sum, gap) => sum + framesLost(gap, frame), 0),
    stalls: gaps.filter((gap) => gap > frame * 1.5).length,
    p50: percentile(gaps, 0.5),
    p95: percentile(gaps, 0.95),
    worst: gaps.length > 0 ? Math.max(...gaps) : 0,
  };
}

/** One frame drawn, and how long the main thread held it. */
export interface Drawn {
  at: number;
  gap: number;
  /** From the start of the frame to the recorder's turn in it: what the page
      did first with the scrolling and the hand. */
  before: number;
  /** From there until the frame was painted: the page's other work for the
      frame, then style, layout and paint. Unknown until it was painted. */
  painting?: number;
}

/** How long the main thread held one frame. */
export function heldFor(drawn: Drawn): number {
  return drawn.before + (drawn.painting ?? 0);
}

/** Below this share of a frame, the main thread had room to spare. */
const ROOM_TO_SPARE = 0.6;

/**
 * Whether the main thread is what made a frame late: the frame before it
 * held it for most of a frame's worth of time, so the next one could not
 * start on time. Otherwise the time went elsewhere: the compositor, the
 * pictures or the graphics card.
 */
export function heldUpByTheMainThread(previous: Drawn | undefined, frame: number): boolean {
  return previous !== undefined && heldFor(previous) > frame * ROOM_TO_SPARE;
}

/** A stretch of time. */
export interface Span {
  from: number;
  to: number;
}

/** The frames drawn while the page was in view: a tab put in the background
 *  stops drawing, and the first frame after it reads as one enormous stall. */
export function framesInView(frames: Drawn[], away: Span[]): Drawn[] {
  return frames.filter(
    (drawn) => !away.some((span) => span.from < drawn.at && span.to > drawn.at - drawn.gap),
  );
}

/** Something the person did, or the page set moving, at one moment. */
export interface Doing {
  at: number;
  what: string;
}

/** The latest of some entries, in order of time, at or before a moment. */
export function latestBefore<T extends { at: number }>(entries: T[], at: number): T | null {
  let low = 0;
  let high = entries.length;
  while (low < high) {
    const middle = (low + high) >> 1;
    if (entries[middle].at <= at) {
      low = middle + 1;
    } else {
      high = middle;
    }
  }
  return low > 0 ? entries[low - 1] : null;
}

/** What went on between two moments, and in the `within` before the first:
 *  each doing once, the latest ones when there are more than `most`. */
export function doingsAround(
  doings: Doing[],
  from: number,
  to: number,
  within: number,
  most: number,
): string[] {
  const kinds: string[] = [];
  for (const doing of doings) {
    if (doing.at >= from - within && doing.at <= to && !kinds.includes(doing.what)) {
      kinds.push(doing.what);
    }
  }
  return kinds.slice(-most);
}

/** One event the page took a while to answer, as the browser describes it. */
export interface Timed {
  startTime: number;
  duration: number;
  processingStart: number;
  processingEnd: number;
}

/** Where the time of an event went: before the page's handlers ran, in them,
 *  and from the end of them until the next frame was drawn. */
export function eventParts(timed: Timed): { waited: number; handling: number; drawing: number } {
  return {
    waited: Math.max(0, timed.processingStart - timed.startTime),
    handling: Math.max(0, timed.processingEnd - timed.processingStart),
    drawing: Math.max(0, timed.startTime + timed.duration - timed.processingEnd),
  };
}

/** An event the page was slow to answer. */
export interface Slow extends Timed {
  name: string;
  what: string;
}

/** Events on one element this close in time are one hand movement: a hover
 *  is a dozen events, all waiting for the same frame. */
const ONE_MOVEMENT_MS = 100;

/** The browser rounds the time of an event to eight milliseconds. */
const ROUNDING_MS = 8;

/** The slow events with those of one movement on one element made one: named
 *  by all their types, timed from the first, which waited the longest, and
 *  handled from the first handler's start to the last one's end. */
export function slowMoments(slow: Slow[]): Slow[] {
  const moments: Slow[] = [];
  const endOf = (one: Slow) => one.startTime + one.duration;
  for (const one of [...slow].sort((a, b) => a.startTime - b.startTime)) {
    let moment: Slow | undefined;
    for (
      let at = moments.length - 1;
      at >= 0 && one.startTime - moments[at].startTime <= ONE_MOVEMENT_MS;
      at -= 1
    ) {
      if (moments[at].what === one.what && Math.abs(endOf(moments[at]) - endOf(one)) <= ROUNDING_MS) {
        moment = moments[at];
        break;
      }
    }
    if (!moment) {
      moments.push({ ...one });
    } else {
      if (!moment.name.split(", ").includes(one.name)) {
        moment.name = `${moment.name}, ${one.name}`;
      }
      moment.processingStart = Math.min(moment.processingStart, one.processingStart);
      moment.processingEnd = Math.max(moment.processingEnd, one.processingEnd);
    }
  }
  return moments;
}

/** The frames drawn while something moved: those that came no later than
 *  `within` after one of the moments it moved, which are in order. */
export function framesWhile(frames: Drawn[], moments: number[], within: number): Drawn[] {
  let next = 0;
  return frames.filter((drawn) => {
    while (next + 1 < moments.length && moments[next + 1] <= drawn.at) {
      next += 1;
    }
    const moved = moments[next];
    return moved !== undefined && moved <= drawn.at && drawn.at - moved <= within;
  });
}
