/*
 * The queue of songs: what is playing, what comes after it, and what shuffle
 * and repeat make of that.
 *
 * Pure, so every rule is tested without a sound being played. The songs are
 * kept in the order they were queued; shuffling draws another order over
 * them, and turning shuffle off goes back to the first one from the song that
 * is playing, as a record player's queue does.
 */

import type { Song } from "../api";

export type Repeat = "off" | "all" | "one";

export interface Queue {
  /** In the order they were queued. */
  songs: Song[];
  /** The order they are played in: places in `songs`. */
  order: number[];
  /** Where in `order` the song playing is. */
  at: number;
  shuffle: boolean;
  repeat: Repeat;
}

export const EMPTY: Queue = { songs: [], order: [], at: 0, shuffle: false, repeat: "off" };

/** The song playing, if any. */
export function current(queue: Queue): Song | null {
  const place = queue.order[queue.at];
  return place === undefined ? null : (queue.songs[place] ?? null);
}

/** The songs still to come, in the order they will play. */
export function upNext(queue: Queue): Song[] {
  return queue.order.slice(queue.at + 1).map((place) => queue.songs[place]);
}

/**
 * A new queue, playing `songs` from the one at `first`. Shuffled, that one
 * plays first and the rest follow in a drawn order; `draw` is what draws it,
 * given so a test can say what comes out.
 */
export function playing(
  songs: Song[],
  first: number,
  shuffle: boolean,
  repeat: Repeat,
  draw: () => number = Math.random,
): Queue {
  const places = songs.map((_, place) => place);
  const start = Math.min(Math.max(first, 0), Math.max(songs.length - 1, 0));
  if (!shuffle) {
    return { songs, order: places, at: start, shuffle, repeat };
  }
  const rest = places.filter((place) => place !== start);
  return { songs, order: [start, ...shuffled(rest, draw)], at: 0, shuffle, repeat };
}

/** Where the queue goes when a song ends on its own: the next one, the same
 *  one again under repeat one, back to the first under repeat all, or
 *  nowhere at the end. */
export function afterTheEnd(queue: Queue): Queue | null {
  if (queue.repeat === "one") {
    return queue;
  }
  return forward(queue);
}

/** Where the queue goes when somebody asks for the next song: always on,
 *  even under repeat one, which is a way to stop repeating that song. */
export function forward(queue: Queue): Queue | null {
  if (queue.at + 1 < queue.order.length) {
    return { ...queue, at: queue.at + 1 };
  }
  if (queue.repeat !== "off" && queue.order.length > 0) {
    return { ...queue, at: 0 };
  }
  return null;
}

/** Whether there is another song to ask for on: one to come, or one to come
 *  back to under repeat. A lone song has none, repeat or not. */
export function canStepOn(queue: Queue): boolean {
  return queue.at + 1 < queue.order.length || (queue.repeat !== "off" && queue.order.length > 1);
}

/** Whether there is another song to go back to: one already played, or the
 *  last one under repeat all. A lone song has none, repeat or not. */
export function canStepBack(queue: Queue): boolean {
  return queue.at > 0 || (queue.repeat === "all" && queue.order.length > 1);
}

/** How far into a song "previous" still means "this one from the start". */
export const BACK_TO_THE_START_SECONDS = 3;

/**
 * Where the queue goes when somebody asks for the previous song. A few
 * seconds into a song, the same song from its start, as every player does;
 * before that, the one before it. Nothing to go back to is the same song
 * from its start.
 */
export function backward(queue: Queue, secondsIn: number): { queue: Queue; restart: boolean } {
  if (secondsIn > BACK_TO_THE_START_SECONDS || queue.at === 0) {
    if (queue.at === 0 && secondsIn <= BACK_TO_THE_START_SECONDS && queue.repeat === "all") {
      return { queue: { ...queue, at: queue.order.length - 1 }, restart: false };
    }
    return { queue, restart: true };
  }
  return { queue: { ...queue, at: queue.at - 1 }, restart: false };
}

/** Shuffle turned on or off, the song playing staying where it is. */
export function withShuffle(queue: Queue, shuffle: boolean, draw: () => number = Math.random): Queue {
  const playingNow = queue.order[queue.at];
  if (playingNow === undefined || shuffle === queue.shuffle) {
    return { ...queue, shuffle };
  }
  if (shuffle) {
    const rest = queue.songs.map((_, place) => place).filter((place) => place !== playingNow);
    return { ...queue, order: [playingNow, ...shuffled(rest, draw)], at: 0, shuffle };
  }
  const order = queue.songs.map((_, place) => place);
  return { ...queue, order, at: playingNow, shuffle };
}

/** The next way round of repeat: off, the whole queue, the one song. */
export function nextRepeat(repeat: Repeat): Repeat {
  return repeat === "off" ? "all" : repeat === "all" ? "one" : "off";
}

/** A jump to one of the songs to come, or already played, by its place in
 *  the order they play in. */
export function jumpTo(queue: Queue, at: number): Queue {
  return at >= 0 && at < queue.order.length ? { ...queue, at } : queue;
}

/** Songs put right after the one playing. */
export function playNext(queue: Queue, songs: Song[]): Queue {
  if (queue.order.length === 0) {
    return playing(songs, 0, false, queue.repeat);
  }
  const added = songs.map((_, index) => queue.songs.length + index);
  const order = [...queue.order.slice(0, queue.at + 1), ...added, ...queue.order.slice(queue.at + 1)];
  return { ...queue, songs: [...queue.songs, ...songs], order };
}

/** Songs put at the end of the queue. */
export function playLast(queue: Queue, songs: Song[]): Queue {
  if (queue.order.length === 0) {
    return playing(songs, 0, false, queue.repeat);
  }
  const added = songs.map((_, index) => queue.songs.length + index);
  return { ...queue, songs: [...queue.songs, ...songs], order: [...queue.order, ...added] };
}

/** Takes one song out of what is still to come, by its place in the order. */
export function without(queue: Queue, at: number): Queue {
  if (at <= queue.at || at >= queue.order.length) {
    return queue;
  }
  return { ...queue, order: queue.order.filter((_, index) => index !== at) };
}

/**
 * A song of the queue put at another place in the order, whichever it is:
 * to come, played or playing. The song playing keeps playing wherever the
 * move leaves it.
 */
export function moved(queue: Queue, from: number, to: number): Queue {
  const last = queue.order.length - 1;
  if (from === to || from < 0 || to < 0 || from > last || to > last) {
    return queue;
  }
  const order = [...queue.order];
  const [place] = order.splice(from, 1);
  order.splice(to, 0, place);
  let at = queue.at;
  if (from === queue.at) {
    at = to;
  } else if (from < queue.at && to >= queue.at) {
    at -= 1;
  } else if (from > queue.at && to <= queue.at) {
    at += 1;
  }
  return { ...queue, order, at };
}

/** An order drawn at random, every one as likely as any other. */
function shuffled(places: number[], draw: () => number): number[] {
  const drawn = [...places];
  for (let index = drawn.length - 1; index > 0; index -= 1) {
    const other = Math.floor(draw() * (index + 1));
    [drawn[index], drawn[other]] = [drawn[other], drawn[index]];
  }
  return drawn;
}
