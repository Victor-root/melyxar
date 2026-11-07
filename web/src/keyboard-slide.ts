/*
 * The keyboard of the Android application on its way up or down. The page's
 * own height only changes once the keyboard has arrived, or as soon as it
 * leaves, so what stands on the foot of the screen follows it from here.
 */

/** A slide as the application says it, the times in milliseconds. */
export interface KeyboardSlide {
  from: number;
  to: number;
  duration: number;
  /** The share of the way covered, evenly sampled in time from 0 to 1. */
  course: readonly number[];
  startedAt: number;
}

/** How high the keyboard stands at a moment of its slide. */
export function keyboardAt(slide: KeyboardSlide, now: number): number {
  const { from, to, duration, course, startedAt } = slide;
  const elapsed = Math.min(Math.max((now - startedAt) / duration, 0), 1);
  const place = elapsed * (course.length - 1);
  const step = Math.floor(place);
  const covered = course[step];
  const next = course[Math.min(step + 1, course.length - 1)];
  return from + (to - from) * (covered + (next - covered) * (place - step));
}

/**
 * What the keyboard takes that the page has not given up yet: its height, less
 * what the window has already lost to it and the navigation bar it rises over.
 */
export function keyboardRise(keyboard: number, windowLost: number, bottomBar: number): number {
  return Math.max(0, keyboard - windowLost - bottomBar);
}
