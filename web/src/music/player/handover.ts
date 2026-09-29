/*
 * When the next song takes over from the one playing: prepared a while
 * before the end, and started just as the end comes, or as early as a
 * crossfade asks for.
 */

/** How long before the end the next song is asked for, so it is there when
 *  its moment comes even on a slow way in. */
export const PREPARE_AHEAD_SECONDS = 20;

/** How long before the moment it is due the next song is started: the time
 *  it takes a browser to start sound that is already there. */
const START_LEAD_SECONDS = 0.02;

/**
 * How long is left of the song playing, in seconds. An element that knows
 * the length of what it plays says so; a song converted on its way does not,
 * and its length is the song's own, counted from where it was asked for.
 */
export function secondsLeft(
  elementDuration: number,
  elementTime: number,
  offset: number,
  songSeconds: number | null,
): number | null {
  if (Number.isFinite(elementDuration) && elementDuration > 0) {
    return Math.max(0, elementDuration - elementTime);
  }
  if (songSeconds === null) {
    return null;
  }
  return Math.max(0, songSeconds - (offset + elementTime));
}

/** How long from now the next song starts, with this much left of the one
 *  playing and this long a crossfade. */
export function handOverIn(left: number, crossfadeSeconds: number): number {
  return Math.max(0, left - crossfadeSeconds - START_LEAD_SECONDS);
}

/** The crossfade a song can take: never more than a third of either song,
 *  so a short one is never all fade. */
export function fadeBetween(crossfadeSeconds: number, fromSeconds: number | null, toSeconds: number | null): number {
  const shortest = Math.min(fromSeconds ?? Infinity, toSeconds ?? Infinity);
  return Math.max(0, Math.min(crossfadeSeconds, shortest / 3));
}
