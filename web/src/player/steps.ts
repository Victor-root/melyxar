/*
 * How far the two step buttons of the player jump.
 */

/**
 * How far a step of the film may go, in seconds, as a viewer chooses it for
 * each of the two buttons.
 *
 * Short, because stepping is for a line of dialogue missed or a title
 * sequence passed over, not for choosing a scene: the bar is there for that.
 * Two digits at most, since the length is written inside the button.
 */
export const STEP_LENGTHS = [5, 10, 15, 30, 60, 90];

/** What both buttons step by until somebody chooses otherwise. */
export const THE_USUAL_STEP = 10;
