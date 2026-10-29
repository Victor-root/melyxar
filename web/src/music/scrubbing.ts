/*
 * A song heard at the pace of a hand dragging the playhead along it, as an
 * editing desk does: slowly when it moves slowly, backwards when it goes
 * back, nothing once it stands still. Each move plays the stretch of the
 * song the hand went over, sped up or slowed down to last as long as the
 * move took, faded in and out so that one does not click against the next.
 *
 * The song is read once, whole, into memory: as one channel and at a lower
 * rate than it was recorded, which is plenty to tell a syllable by, and
 * backwards too the first time the hand goes back.
 */

import { soundOf } from "./player/player";

/** The fastest and the slowest the song is heard while dragged. */
const FASTEST = 4;
const SLOWEST = 0.05;

/** How long a move is taken to have lasted at most and at least, in
 *  milliseconds: a hand that rested before moving again did not mean the
 *  song to crawl through the rest. */
const LONGEST_MOVE_MS = 120;
const SHORTEST_MOVE_MS = 8;

/** How far behind the hand the sound may fall before it catches up. */
const MOST_BEHIND_SECONDS = 0.15;

/** How long each stretch fades in and out. */
const FADE_SECONDS = 0.005;

/** The rate the song is read at: a voice is told well below it. */
const READ_AT_RATE = 22_050;

/** One stretch of the song heard for one move of the hand. */
export interface Grain {
  /** Where it starts, in seconds, in the song read forwards or backwards. */
  offset: number;
  /** How much of the song it covers, in seconds. */
  covers: number;
  rate: number;
  backwards: boolean;
  /** How long it lasts once heard at its rate, in seconds. */
  lasts: number;
}

/** What a move from `from` to `to`, in seconds of a song `length` long,
 *  taking `elapsedMs`, plays: nothing for a hand that did not move. */
export function grainOf(from: number, to: number, elapsedMs: number, length: number): Grain | null {
  const moved = to - from;
  if (Math.abs(moved) < 0.002) {
    return null;
  }
  const took = Math.min(Math.max(elapsedMs, SHORTEST_MOVE_MS), LONGEST_MOVE_MS) / 1000;
  const covers = Math.abs(moved);
  const rate = Math.min(Math.max(covers / took, SLOWEST), FASTEST);
  const backwards = moved < 0;
  return { offset: backwards ? length - from : from, covers, rate, backwards, lasts: covers / rate };
}

export class Scrubber {
  private context: AudioContext | null = null;
  private forwards: AudioBuffer | null = null;
  private backwards: AudioBuffer | null = null;
  private reading: Promise<void> | null = null;
  private stop = new AbortController();
  private at = 0;
  private movedAt = 0;
  /** When the last stretch scheduled ends, on the clock of the sound. */
  private heardUntil = 0;

  constructor(private readonly song: string) {}

  /** Reads the song into memory, once, ahead of the first drag. */
  read(): void {
    this.reading ??= (async () => {
      const answer = await fetch(soundOf(this.song, 0), { signal: this.stop.signal });
      const bytes = await answer.arrayBuffer();
      const context = new AudioContext({ sampleRate: READ_AT_RATE });
      this.context = context;
      const read = await context.decodeAudioData(bytes);
      this.forwards = oneChannel(context, read);
    })().catch(() => {
      // Not heard while dragged, and dragged all the same.
    });
  }

  /** The hand took the playhead at `at` seconds. */
  begin(at: number): void {
    this.at = at;
    this.movedAt = performance.now();
    void this.context?.resume();
  }

  /** The hand moved the playhead to `at` seconds. */
  to(at: number): void {
    const now = performance.now();
    const context = this.context;
    const forwards = this.forwards;
    const grain = forwards ? grainOf(this.at, at, now - this.movedAt, forwards.duration) : null;
    this.at = at;
    this.movedAt = now;
    if (!context || !forwards || !grain) {
      return;
    }
    if (grain.backwards) {
      this.backwards ??= backwardsOf(context, forwards);
    }
    const source = context.createBufferSource();
    source.buffer = grain.backwards ? this.backwards : forwards;
    source.playbackRate.value = grain.rate;
    const fading = context.createGain();
    const fade = Math.min(FADE_SECONDS, grain.lasts / 4);
    let when = Math.max(context.currentTime, this.heardUntil - fade);
    if (when - context.currentTime > MOST_BEHIND_SECONDS) {
      when = context.currentTime;
    }
    fading.gain.setValueAtTime(0, when);
    fading.gain.linearRampToValueAtTime(1, when + fade);
    fading.gain.setValueAtTime(1, when + grain.lasts - fade);
    fading.gain.linearRampToValueAtTime(0, when + grain.lasts);
    source.connect(fading).connect(context.destination);
    source.start(when, grain.offset, grain.covers);
    this.heardUntil = when + grain.lasts;
  }

  /** Lets go of the song for good. */
  close(): void {
    this.stop.abort();
    void this.context?.close();
    this.context = null;
    this.forwards = null;
    this.backwards = null;
  }
}

/** The song as one channel, its channels heard together. */
function oneChannel(context: AudioContext, read: AudioBuffer): AudioBuffer {
  const one = context.createBuffer(1, read.length, read.sampleRate);
  const into = one.getChannelData(0);
  for (let channel = 0; channel < read.numberOfChannels; channel += 1) {
    const from = read.getChannelData(channel);
    for (let at = 0; at < from.length; at += 1) {
      into[at] += from[at] / read.numberOfChannels;
    }
  }
  return one;
}

function backwardsOf(context: AudioContext, forwards: AudioBuffer): AudioBuffer {
  const backwards = context.createBuffer(1, forwards.length, forwards.sampleRate);
  const from = forwards.getChannelData(0);
  const into = backwards.getChannelData(0);
  for (let at = 0; at < from.length; at += 1) {
    into[at] = from[from.length - 1 - at];
  }
  return backwards;
}
