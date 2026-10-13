/*
 * Which offers of lyrics are missing lines, told by comparing them with each
 * other, and which were stamped on a song of the length of the file.
 *
 * A long stretch between two stamped lines says nothing on its own: a song has
 * its instrumental breaks, and every version of it has them. It says something
 * when another version of the same song has no stretch so long, since this one
 * then leaves the song running with nothing lit where the other has words.
 */

import type { LyricsOffer } from "./api";

/** How far from the length of the file a stamped version still lines up with
    it, in milliseconds, as the server holds it when it chooses on its own. */
export const SAME_LENGTH_MS = 3_000;

/** How far an offer's length is from the file's, in milliseconds, when both
    are known. */
export function lengthGap(offer: LyricsOffer, durationMs: number | null): number | null {
  return offer.duration_ms === null || durationMs === null ? null : offer.duration_ms - durationMs;
}

/** How much longer than the best version's longest stretch is worth saying, in
    seconds: the same break stamped a little differently is not a missing line. */
export const EXTRA_GAP_SECONDS = 10;

/** The offers whose longest stretch without a line is clearly longer than the
    shortest of the stamped offers, by their ids. */
export function offersMissingLines(offers: LyricsOffer[]): Set<number> {
  const gaps = offers.flatMap((offer) => (offer.synced && offer.longest_gap_seconds !== null ? [offer.longest_gap_seconds] : []));
  if (gaps.length < 2) {
    return new Set();
  }
  const shortest = Math.min(...gaps);
  return new Set(
    offers
      .filter((offer) => offer.synced && offer.longest_gap_seconds !== null && offer.longest_gap_seconds - shortest >= EXTRA_GAP_SECONDS)
      .map((offer) => offer.id),
  );
}
