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
