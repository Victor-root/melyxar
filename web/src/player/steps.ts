/*
 * How far the two step buttons of the player jump.
 */

/**
 * The lengths offered for a step of the film, in seconds, for each of the
 * two buttons and for the way back on resuming. Anything else up to the
 * server's longest is typed in by hand.
 *
 * Short, because stepping is for a line of dialogue missed or a title
 * sequence passed over, not for choosing a scene: the bar is there for that.
 */
export const STEP_LENGTHS = [5, 10, 15, 20, 25, 30];

/** What the button back steps by until somebody chooses: a line heard again. */
export const THE_USUAL_STEP_BACK = 10;

/** What the button on steps by until somebody chooses: a stretch passed over. */
export const THE_USUAL_STEP_ON = 30;

/** What a length picker shows for a length typed in by hand. */
export const TYPED_IN = "custom";

/**
 * What a picker of lengths shows for a length: the length itself when it is
 * one of those offered, nought too where "none" is offered, and "typed in by
 * hand" for anything else.
 */
export function offeredAs(seconds: number, noneOffered: boolean): string {
  return STEP_LENGTHS.includes(seconds) || (noneOffered && seconds === 0)
    ? String(seconds)
    : TYPED_IN;
}
