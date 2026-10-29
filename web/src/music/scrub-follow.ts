/*
 * The sound of a song under a hand dragging the playhead along it, as an
 * editing desk plays it: the song read at the speed the hand moves, a tape
 * sped up, slowed down and run backwards, worked out on the thread that
 * makes the sound (scrub-worklet.ts) one block of samples at a time.
 *
 * A screen sees a hand in whole pixels, a few times a frame at best: a slow
 * drag is a pixel now and then, and a speed read from the gap between the
 * hand and the sound jumps at every pixel and falls back between two, which
 * is heard as noise rather than as a voice. So the hand is followed by its
 * path instead: every point it was seen at, with the moment it was there.
 * Followed a little late, the song is always between two points the hand
 * went through, and between two points it moved at a known speed. The speed
 * is that of the path over the last stretch, eased; the sound keeps within
 * a small gap of the path, is pulled along past it and jumps past a large
 * one, and is heard only while the hand moves.
 */

/** Where the hand was, in seconds of the song, and when, in seconds of the
 *  clock the sound is made by, as the sound is heard. */
export interface HandPoint {
  at: number;
  time: number;
}

/** Over how long the speed of the path is read, in seconds. */
const SPEED_OVER = 0.08;

/** How soon the speed of the sound follows the speed read, in seconds. */
const SPEED_EASE = 0.02;

/** How far the sound may stray from the path before it is pulled along,
 *  how soon it is pulled back, and how far it may stray before it jumps,
 *  all in seconds. */
const LEEWAY = 0.06;
const PULL = 0.15;
const JUMP_OVER = 0.4;

/** The fastest the song is heard, either way. */
const FASTEST = 4;

/** How fast the hand goes before it is heard at all, and before it is heard
 *  in full: a hand nearly still is heard as nothing rather than a growl. */
const HEARD_FROM = 0.02;
const HEARD_FULLY_FROM = 0.15;

/** How long the sound takes to come in or go out, in seconds. */
const FADE = 0.006;

/** How long the path is kept behind its latest point, in seconds: longer
 *  than it is ever followed late, and its speed read, put together. */
const KEPT_SECONDS = 1;

export class ScrubFollower {
  private path: HandPoint[] = [];
  /** How late the path is followed, in seconds. */
  private late = 0.06;
  private held = false;
  /** Where the sound reads, in seconds of the song, how fast it goes and
      how loud it is heard. */
  private head = 0;
  private read = 0;
  private level = 0;
  /** The speed the sound had at the end of the last block, where the next
      one takes it up. */
  private going = 0;
  private readonly length: number;

  constructor(
    private readonly sound: Float32Array,
    private readonly soundRate: number,
    private readonly outRate: number,
  ) {
    this.length = sound.length / soundRate;
  }

  /** The hand took the playhead: the sound starts from there. */
  take(point: HandPoint, late: number): void {
    this.path = [point];
    this.late = late;
    this.head = point.at;
    this.read = 0;
    this.going = 0;
    this.held = true;
  }

  /** Where the hand went since. */
  move(points: readonly HandPoint[]): void {
    if (!this.held) {
      return;
    }
    const path = this.path;
    for (const point of points) {
      if (point.time > path[path.length - 1].time) {
        path.push(point);
      }
    }
    const kept = path[path.length - 1].time - KEPT_SECONDS;
    let gone = 0;
    while (gone < path.length - 2 && path[gone + 1].time < kept) {
      gone += 1;
    }
    path.splice(0, gone);
  }

  /** The hand let go: the sound goes out. */
  letGo(): void {
    this.held = false;
  }

  /** Fills a block of sound, whose first sample is heard at `time`. */
  render(out: Float32Array, time: number): void {
    if (this.path.length === 0) {
      out.fill(0);
      return;
    }
    const lasts = out.length / this.outRate;
    const followed = time + lasts - this.late;
    const target = this.where(followed);
    const wanted = (target - this.where(followed - SPEED_OVER)) / SPEED_OVER;
    this.read += (wanted - this.read) * (1 - Math.exp(-lasts / SPEED_EASE));

    let gap = target - this.head;
    let level = this.level;
    if (Math.abs(gap) > JUMP_OVER) {
      this.head = target - Math.sign(gap) * LEEWAY / 2;
      gap = target - this.head;
      level = 0;
    }
    const beyond = Math.sign(gap) * Math.max(0, Math.abs(gap) - LEEWAY);
    const speedFrom = this.going;
    const speedTo = clamp(this.read + beyond / PULL, -FASTEST, FASTEST);
    const loud = this.held ? clamp((Math.abs(this.read) - HEARD_FROM) / (HEARD_FULLY_FROM - HEARD_FROM), 0, 1) : 0;
    const easing = 1 - Math.exp(-1 / (FADE * this.outRate));
    const n = out.length;
    for (let i = 0; i < n; i += 1) {
      const speed = speedFrom + ((speedTo - speedFrom) * (i + 1)) / n;
      this.head = clamp(this.head + speed / this.outRate, 0, this.length);
      level += (loud - level) * easing;
      out[i] = level * sampleAt(this.sound, this.head * this.soundRate);
    }
    this.level = level;
    this.going = speedTo;
    if (!this.held && level < 1e-4) {
      this.path = [];
    }
  }

  /** Where the hand was at `time`: between two points it went through, at
   *  the speed it went from one to the other; past the last, on at its last
   *  speed for as long as it took to get there from the one before, which
   *  is a pixel at most, then still. */
  private where(time: number): number {
    const path = this.path;
    const first = path[0];
    if (time <= first.time || path.length === 1) {
      return first.at;
    }
    for (let index = path.length - 1; index > 0; index -= 1) {
      const before = path[index - 1];
      const after = path[index];
      if (time >= before.time && time <= after.time) {
        return before.at + ((after.at - before.at) * (time - before.time)) / (after.time - before.time);
      }
    }
    const before = path[path.length - 2];
    const last = path[path.length - 1];
    const took = last.time - before.time;
    const ahead = Math.min(time - last.time, took);
    return last.at + ((last.at - before.at) / took) * ahead;
  }
}

function clamp(value: number, low: number, high: number): number {
  return Math.min(Math.max(value, low), high);
}

/** The sound between two of its samples, on the smooth curve through the
 *  four around it. */
export function sampleAt(sound: Float32Array, at: number): number {
  const index = Math.floor(at);
  const share = at - index;
  const last = sound.length - 1;
  const a = sound[clamp(index - 1, 0, last)];
  const b = sound[clamp(index, 0, last)];
  const c = sound[clamp(index + 1, 0, last)];
  const d = sound[clamp(index + 2, 0, last)];
  const slope = 0.5 * (c - a);
  const bend = a - 2.5 * b + 2 * c - 0.5 * d;
  const twist = 0.5 * (d - a) + 1.5 * (b - c);
  return ((twist * share + bend) * share + slope) * share + b;
}
