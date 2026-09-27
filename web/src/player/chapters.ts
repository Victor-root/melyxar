/*
 * Where a film stands among its chapters, and where the two chapter buttons
 * take it: the bar of controls and the drawer's list of scenes ask the same
 * question and must never answer it differently.
 */

import type { PlaybackChapter } from "../api";

/** How far back a moment may be read as the start of the chapter after it: a
 *  film sent to a chapter lands a hair before its first frame. */
const LANDED_WITHIN = 0.25;

/** How long after a chapter starts a press on "back" still goes to the one
 *  before rather than to its start. */
const JUST_BEGUN = 3;

/** Which chapter a moment falls inside, or -1 before the first. */
export function chapterAt(chapters: PlaybackChapter[], at: number): number {
  let inside = -1;
  for (let index = 0; index < chapters.length; index += 1) {
    if (chapters[index].at_second <= at + LANDED_WITHIN) {
      inside = index;
    }
  }
  return inside;
}

/**
 * Where a chapter button takes the film, and whether there is anywhere to go.
 *
 * Back goes to the start of this chapter unless it has only just begun, which
 * is what every player does and what a hand expects: one press to replay the
 * scene, two to reach the one before. Before the first chapter it goes to the
 * very start, and after the last, forward goes to the very end.
 */
export function chapterStep(
  chapters: PlaybackChapter[],
  at: number,
  length: number,
  back: boolean,
): { to: number; possible: boolean } {
  const on = chapterAt(chapters, at);
  const justBegun = on >= 0 && at - chapters[on].at_second < JUST_BEGUN;
  const wanted = back ? (justBegun ? on - 1 : on) : on + 1;
  const there = chapters[wanted];
  return {
    to: there ? there.at_second : back ? 0 : length,
    possible: back ? !(on < 0 && at < JUST_BEGUN) : wanted < chapters.length,
  };
}
