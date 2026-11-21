/*
 * Film the library counts as held when it never fetched it.
 *
 * The library takes a gap in what the browser holds as nothing at all when it
 * is narrower than a seam, and counts the film on both sides as one stretch.
 * That is right for a seam, where two readings of the film meet a few
 * hundredths apart and the segments on either side follow one another. It is
 * wrong when the gap is made of segments nobody fetched: a film cut where its
 * own key frames lie can have segments of a few hundredths, three of them fit
 * in a gap narrower than a seam, and the library, counting them as held, never
 * asks for them. It then reaches the gap, will not hop over film it never had,
 * and stands still until the viewer pauses and plays again. Seen on an episode
 * after a few quick clicks on the bar: three segments of eight hundredths left
 * out, and the film stood still for good.
 *
 * Told apart by the segments' numbers, never by where the playlist places
 * them: a segment not yet fetched sits where the library guesses, and on a
 * film it reads badly that guess was measured more than half a second out.
 * Where a segment it has fetched lies, the browser has said.
 */

/** A stretch of the film's clock, in seconds. */
export type Stretch = { start: number; end: number };

/** The segments fetched, by number, each where its film lies. */
export type Fetched = ReadonlyMap<number, Stretch>;

/** What the library says of a segment it has handed to the browser. */
type Handed = {
  start: number;
  end: number;
  elementaryStreams: Partial<Record<string, { startPTS: number; endPTS: number } | null>>;
};

/** Where the film of a handed segment lies, as the browser placed it. */
export function whereItLies(segment: Handed): Stretch {
  const streams = segment.elementaryStreams;
  const placed = streams.video ?? streams.audiovideo ?? streams.audio;
  return placed ? { start: placed.startPTS, end: placed.endPTS } : { start: segment.start, end: segment.end };
}

/** What the browser holds, as stretches. */
export function stretchesHeld(ranges: TimeRanges): Stretch[] {
  const held: Stretch[] = [];
  for (let index = 0; index < ranges.length; index += 1) {
    held.push({ start: ranges.start(index), end: ranges.end(index) });
  }
  return held;
}

/** Film left out just ahead: where it begins, and the segment it follows. */
export type LeftOut = { from: number; after: number };

/**
 * The segments never fetched inside the gap just ahead of the film at `at`,
 * when that gap is narrow enough for the library to count it as held. Null
 * when it is a seam, when it is wide enough for the library to see it, or
 * when the segments on either side are not known.
 *
 * Only the gap just ahead: that is the one the film will reach, and throwing
 * away everything past it takes every further one with it.
 */
export function filmLeftOutAhead(
  held: readonly Stretch[],
  at: number,
  fetched: Fetched,
  widestSeam: number,
): LeftOut | null {
  const index = held.findIndex((stretch) => stretch.start <= at && at <= stretch.end);
  if (index < 0) {
    return null;
  }
  const here = held[index];
  const next = held[index + 1];
  if (!next || next.start - here.end > widestSeam) {
    return null;
  }
  const before = closest(fetched, (stretch) => stretch.start < here.end, (stretch) => -Math.abs(stretch.end - here.end));
  const after = closest(fetched, (stretch) => stretch.end > next.start, (stretch) => -Math.abs(stretch.start - next.start));
  if (before === null || after === null) {
    return null;
  }
  for (let sn = before + 1; sn < after; sn += 1) {
    if (!fetched.has(sn)) {
      return { from: here.end, after: before };
    }
  }
  return null;
}

/** The fetched segment that scores best among those that qualify. */
function closest(
  fetched: Fetched,
  qualifies: (stretch: Stretch) => boolean,
  score: (stretch: Stretch) => number,
): number | null {
  let best: number | null = null;
  let bestScore = Number.NEGATIVE_INFINITY;
  for (const [sn, stretch] of fetched) {
    if (qualifies(stretch) && score(stretch) > bestScore) {
      best = sn;
      bestScore = score(stretch);
    }
  }
  return best;
}

/** Allowance for the rounding between the library's times and the browser's. */
const ROUNDING = 0.001;

/** Forgets the segments whose film was thrown away between two moments. */
export function forgetThrownAway(fetched: Map<number, Stretch>, from: number, to: number) {
  for (const [sn, stretch] of fetched) {
    if (stretch.end > from + ROUNDING && stretch.start < to - ROUNDING) {
      fetched.delete(sn);
    }
  }
}
