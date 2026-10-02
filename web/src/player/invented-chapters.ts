/*
 * The chapters a film's page shows: the ones its file names, or, when it names
 * none, moments laid at an even distance along it, each with the little
 * picture the bar of the player already has for it.
 *
 * Invented ones are only ever offered where there are pictures to put on them,
 * and never reach the player: its bar and its jumps read the chapters the file
 * names and nothing else.
 */

import type { PlaybackChapter, PlaybackPlan } from "../api";

/** The distances an invented chapter may stand at, in seconds, from the
 *  shortest: round figures, so the times on the cards read as a rhythm. */
const ROUND_STEPS = [300, 600, 900, 1200, 1800, 3600];
/** The most cards a row of them is allowed, and the fewest that make a row. */
const MOST_CARDS = 12;
const FEWEST_CARDS = 3;
/** How close to the end of the film a moment may not be: a card on the
 *  credits' last seconds shows nothing worth a press. */
const CLEAR_OF_THE_END = 60;

/** Where invented chapters stand along a film of this length: at its start
 *  and then every round distance, the shortest that keeps them to a row of
 *  a reasonable size. None where the film is too short to make a row. */
export function inventedMoments(length: number): number[] {
  if (!(length > 0)) {
    return [];
  }
  const step = ROUND_STEPS.find((one) => Math.ceil(length / one) <= MOST_CARDS) ?? ROUND_STEPS[ROUND_STEPS.length - 1];
  const moments: number[] = [];
  for (let at = 0; at < length - CLEAR_OF_THE_END; at += step) {
    moments.push(at);
  }
  return moments.length >= FEWEST_CARDS ? moments : [];
}

/** How long the film is, as far as the plan knows: the pictures cover the
 *  film as far as they go, and the length the file says is kept when it is
 *  shorter. */
function lengthOf(plan: PlaybackPlan): number {
  const covered = plan.thumbnails ? plan.thumbnails.counted * plan.thumbnails.every_seconds : 0;
  const said = plan.duration_minutes !== null ? plan.duration_minutes * 60 : 0;
  return said > 0 && covered > 0 ? Math.min(said, covered) : Math.max(said, covered);
}

/** The chapters to show for a film: those its file names, or invented ones
 *  when it names none and its pictures are there to put on them. */
export function chaptersOf(plan: PlaybackPlan): PlaybackChapter[] {
  if (plan.chapters.length > 0) {
    return plan.chapters;
  }
  if (!plan.thumbnails) {
    return [];
  }
  return inventedMoments(lengthOf(plan)).map((at_second) => ({ at_second, title: null }));
}
