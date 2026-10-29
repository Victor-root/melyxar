import { describe, expect, it } from "vitest";
import { ScrubFollower, sampleAt } from "./scrub-follow";
import type { HandPoint } from "./scrub-follow";

const SONG_RATE = 22_050;
const OUT_RATE = 48_000;
const BLOCK = 128;
/** Thirty seconds over a thousand pixels, on a screen at 125 %. */
const SECONDS_A_PIXEL = 30 / 1000 / 1.25;
const LATE = 0.06;

/** A steady 440 Hz tone, a minute long. */
const tone = Float32Array.from({ length: 60 * SONG_RATE }, (_, i) => 0.8 * Math.sin((2 * Math.PI * 440 * i) / SONG_RATE));

/** A hand moving from ten seconds in, leg after leg of [seconds, speed]. */
function handAt(legs: [number, number][], time: number): number {
  let at = 10;
  let start = 0;
  for (const [lasts, speed] of legs) {
    if (time <= start + lasts) {
      return at + speed * (time - start);
    }
    at += speed * lasts;
    start += lasts;
  }
  return at;
}

/** The sound of a hand seen as a browser sees it: whole pixels reported at
 *  125 Hz when it moved, handed over once a frame, worked out a little
 *  before it is heard. */
function heard(legs: [number, number][], seconds: number, letGoAt = Infinity): Float32Array {
  let seed = 7;
  const jitter = () => (seed = (seed * 1_103_515_245 + 12_345) % 2_147_483_648) / 2_147_483_648;
  const raw: HandPoint[] = [];
  let pixel: number | null = null;
  for (let time = 0; time <= Math.min(seconds, letGoAt); time += 1 / 125) {
    const now = Math.round(handAt(legs, time) / SECONDS_A_PIXEL);
    if (now !== pixel) {
      raw.push({ at: now * SECONDS_A_PIXEL, time: time + jitter() * 0.001 });
      pixel = now;
    }
  }
  const frames: { known: number; points: HandPoint[] }[] = [];
  let next = 0;
  for (let frame = 0; frame <= seconds; frame += 1 / 60) {
    const points: HandPoint[] = [];
    while (next < raw.length && raw[next].time <= frame) {
      points.push(raw[next]);
      next += 1;
    }
    if (points.length > 0) {
      frames.push({ known: frame + 0.003 + jitter() * 0.004, points });
    }
  }
  const follower = new ScrubFollower(tone, SONG_RATE, OUT_RATE);
  const out = new Float32Array(Math.round(seconds * OUT_RATE));
  let taken = false;
  let handed = 0;
  for (let block = 0; block * BLOCK < out.length; block += 1) {
    const time = (block * BLOCK) / OUT_RATE;
    const workedOutAt = time - 0.02 - ((block % 4) * BLOCK) / OUT_RATE;
    while (handed < frames.length && frames[handed].known <= workedOutAt) {
      const { points } = frames[handed];
      if (!taken) {
        follower.take(points[0], LATE);
        taken = true;
      }
      follower.move(points);
      handed += 1;
    }
    if (taken && time >= letGoAt + LATE) {
      follower.letGo();
    }
    follower.render(out.subarray(block * BLOCK, Math.min((block + 1) * BLOCK, out.length)), time);
  }
  return out;
}

/** The pitch of every cycle of the sound after `from` seconds. */
function pitches(out: Float32Array, from: number): number[] {
  const found: number[] = [];
  let last: number | null = null;
  for (let i = Math.round(from * OUT_RATE) + 1; i < out.length; i += 1) {
    if (out[i - 1] < 0 && out[i] >= 0 && Math.abs(out[i - 1] - out[i]) > 1e-4) {
      const crossing = i - 1 + out[i - 1] / (out[i - 1] - out[i]);
      if (last !== null) {
        found.push(OUT_RATE / (crossing - last));
      }
      last = crossing;
    }
  }
  return found;
}

/** How much each cycle strays from the one before, the median. */
function warble(found: number[]): number {
  const strays = found.slice(1).map((pitch, index) => Math.abs(pitch - found[index]) / pitch).sort((a, b) => a - b);
  return strays[Math.floor(strays.length / 2)];
}

const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];

/** The last moment anything is heard, in seconds. */
function lastHeard(out: Float32Array): number {
  for (let i = out.length - 1; i >= 0; i -= 1) {
    if (Math.abs(out[i]) > 0.01) {
      return i / OUT_RATE;
    }
  }
  return 0;
}

describe("ScrubFollower", () => {
  it("plays a hand going at the speed of the song at its own pitch, without a warble", () => {
    const found = pitches(heard([[0.3, 0], [2.5, 1]], 2.8), 0.8);
    expect(median(found)).toBeCloseTo(440, -1);
    expect(warble(found)).toBeLessThan(0.005);
  });

  it("plays a hand going ten times slower ten times lower, as smoothly, though it moves a pixel at a time", () => {
    const found = pitches(heard([[0.3, 0], [3, 0.1]], 3.3), 1);
    expect(median(found)).toBeCloseTo(44, 0);
    expect(warble(found)).toBeLessThan(0.005);
  });

  it("plays backwards a hand going back", () => {
    const out = heard([[0.3, 0], [1, 0.5], [1.5, -0.5]], 2.8);
    expect(median(pitches(out, 1.8))).toBeCloseTo(220, -1);
  });

  it("is silent while the hand holds the playhead still", () => {
    const out = heard([[2, 0]], 2);
    expect(lastHeard(out)).toBe(0);
  });

  it("stops soon after the hand stops, and soon after a flick across the song", () => {
    expect(lastHeard(heard([[0.3, 0], [1, 0.8], [1.5, 0]], 2.8))).toBeLessThan(1.3 + 0.35);
    expect(lastHeard(heard([[0.3, 0], [0.25, 24], [1.5, 0]], 2.05))).toBeLessThan(0.55 + 0.45);
  });

  it("goes quiet once the hand lets go, though it was moving", () => {
    expect(lastHeard(heard([[0.3, 0], [3, 1]], 3.3, 1.5))).toBeLessThan(1.5 + LATE + 0.05);
  });
});

describe("sampleAt", () => {
  it("is the sample itself on a sample, and lies on the curve between two", () => {
    const line = Float32Array.from([0, 1, 2, 3, 4]);
    expect(sampleAt(line, 2)).toBe(2);
    expect(sampleAt(line, 2.5)).toBeCloseTo(2.5, 9);
    expect(sampleAt(line, -3)).toBe(0);
    expect(sampleAt(line, 10)).toBe(4);
  });
});
