/**
 * How much of a sixteen by nine picture a banner of this height keeps.
 *
 * The height is a share of the width and the picture is sixteen by nine, so
 * the two divide into each other and no measurement of the window comes into
 * it. It counted the bar at the top as well while the bar was drawn over the
 * banner; once the bar went back to a band of its own it was reading about
 * five too high on an ordinary screen, and saying that a banner kept three
 * fifths of a picture when it kept a little over a half.
 */
export function shareOfThePictureKept(height: number): number {
  return Math.round(Math.min(1, height * (16 / 9)) * 100);
}
