/*
 * What the connection to the server carries, measured before every film.
 *
 * A block is downloaded and timed from its first byte to its last, which
 * leaves out the time the question took to reach the server and come back:
 * that time says how far away the server is, not how much the connection
 * carries. A first small block says roughly how fast the connection is, and a
 * fast one is then given a bigger block, since a small block on a fast
 * connection is over before it can be timed.
 *
 * Never taken from an earlier film: a connection changes between two films,
 * and a speed kept from one played at home started the next one as it is
 * over a slow connection that could not carry it.
 */

import { api } from "../api";

/** The first block, timed on any connection in well under a second. */
const FIRST_BLOCK = 256 * 1024;
/** A first block over this quickly says the connection is worth a second,
 *  bigger one. */
const QUICK_S = 0.25;
/** How long the second block should take, and its bounds. */
const SECOND_BLOCK_S = 0.75;
const SECOND_BLOCK_LEAST = 1024 * 1024;
const SECOND_BLOCK_MOST = 16 * 1024 * 1024;
/** Past this, a measure is given up and nothing is known. */
const PATIENCE_MS = 4_000;
/** Below this, the time between the first byte and the last is too short to
 *  mean anything, and the whole wait is timed instead. */
const TIMED_AT_LEAST_S = 0.05;

/** How long one block took and what it says of the connection. */
interface Timed {
  bitsPerSecond: number;
  seconds: number;
}

async function timeBlock(bytes: number, signal: AbortSignal): Promise<Timed> {
  const asked = performance.now();
  const answer = await api.blockToTime(bytes, signal);
  const reader = answer.body?.getReader();
  if (!reader) {
    throw new Error("the block came with no body");
  }
  let firstAt: number | null = null;
  let afterTheFirst = 0;
  let total = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) {
      break;
    }
    total += value.byteLength;
    if (firstAt === null) {
      firstAt = performance.now();
    } else {
      afterTheFirst += value.byteLength;
    }
  }
  const ended = performance.now();
  const flowing = firstAt === null ? 0 : (ended - firstAt) / 1000;
  const seconds = (ended - asked) / 1000;
  const bitsPerSecond =
    flowing >= TIMED_AT_LEAST_S ? (afterTheFirst * 8) / flowing : (total * 8) / Math.max(seconds, 0.001);
  return { bitsPerSecond, seconds };
}

/** What the connection carries in bits a second, or null when it could not
 *  be measured. */
export async function connectionSpeed(): Promise<number | null> {
  const controller = new AbortController();
  const patience = window.setTimeout(() => controller.abort(), PATIENCE_MS);
  try {
    const first = await timeBlock(FIRST_BLOCK, controller.signal);
    let speed = first.bitsPerSecond;
    if (first.seconds < QUICK_S) {
      const bytes = Math.min(
        SECOND_BLOCK_MOST,
        Math.max(SECOND_BLOCK_LEAST, Math.round((speed / 8) * SECOND_BLOCK_S)),
      );
      speed = (await timeBlock(bytes, controller.signal)).bitsPerSecond;
    }
    return speed;
  } catch {
    return null;
  } finally {
    window.clearTimeout(patience);
  }
}
