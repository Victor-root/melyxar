/*
 * Putting the page where a letter chosen on the rail begins.
 *
 * Shared by the grids of the films and of the music, which jump to a letter
 * in the same way.
 */

import { barRoom } from "./bar-room";

/** How many frames in a row a jump has to stand where it was sent before it
 *  counts as there: enough for the bar at the top to have answered it. */
const STILL_FRAMES = 6;
/** The most frames a jump is given to settle, under a second: long enough for
 *  the cards around it to be drawn, and never a page that fights a hand. */
const LANDING_FRAMES = 45;

/** The box a page scrolls in, which is not the window here: the nearest one
 *  above the element that can be scrolled up and down. */
export function scrollerOf(element: HTMLElement): HTMLElement | null {
  for (let box = element.parentElement; box; box = box.parentElement) {
    const { overflowY } = getComputedStyle(box);
    if (overflowY === "auto" || overflowY === "scroll") {
      return box;
    }
  }
  return null;
}

/**
 * Scrolls `box` until `target` stands at the top of the screen, under the bar
 * at the top while it is out, at the very top once it has stepped aside. Which
 * one it is depends on the jump itself, since the bar steps aside when the
 * page is read down and comes back when it is read up, so it is read again on
 * every frame rather than guessed.
 *
 * Straight there rather than gliding, and put there again on every frame
 * until it holds still. The cards off the screen are not drawn and stand at a
 * guessed height, so where the row is only settles as the cards around it are
 * drawn: a glide drew them one after the other on the way and ended short of
 * it, and a single second look came before they had been.
 *
 * Calls `done` with where the page stands once it has settled, and returns
 * what stops it early.
 */
export function landOn(target: HTMLElement, box: HTMLElement, done: (scrollTop: number) => void): () => void {
  const style = getComputedStyle(box);
  const air = parseFloat(style.getPropertyValue("--gap-wide")) || 0;
  const below = () => target.getBoundingClientRect().top - box.getBoundingClientRect().top;
  let frames = 0;
  let still = 0;
  let next = 0;
  const land = () => {
    const off = below() - barRoom() - air;
    if (Math.abs(off) > 1) {
      box.scrollBy({ top: off, behavior: "instant" });
      still = 0;
    } else {
      still += 1;
    }
    frames += 1;
    if (still < STILL_FRAMES && frames < LANDING_FRAMES) {
      next = requestAnimationFrame(land);
    } else {
      done(box.scrollTop);
    }
  };
  land();
  return () => cancelAnimationFrame(next);
}
