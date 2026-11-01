/*
 * One deck of the player of music: the element of sound a song plays in.
 */

export type Deck = HTMLAudioElement;

/** One deck, asking only for what it needs to start. */
export function aDeck(): Deck {
  const element = new Audio();
  element.preload = "metadata";
  return element;
}
