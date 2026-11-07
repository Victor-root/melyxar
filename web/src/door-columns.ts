/*
 * How many faces stand across on a phone at the door, so that they make a
 * grid rather than a line with one left over, as far as the width allows.
 */

/** The fewest columns that fill their lines, up to as many as a phone has
 *  room for. Past that, four, the rest of a long list on a shorter last line. */
export function columnsOfFaces(faces: number): number {
  if (faces <= 3) {
    return Math.max(faces, 1);
  }
  if (faces === 4) {
    return 2;
  }
  if (faces <= 9 && faces % 3 === 0) {
    return 3;
  }
  if (faces % 4 === 0) {
    return 4;
  }
  if (faces === 5) {
    return 3;
  }
  return faces === 10 ? 5 : 4;
}
