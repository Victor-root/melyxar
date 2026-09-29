/*
 * When a song has been listened to, for its count: once half of it has
 * played, or four minutes of a long one, and never for a song too short to
 * say anything, as the servers people come from count.
 */

/** Below this, in seconds, a song is never counted. */
export const TOO_SHORT_SECONDS = 30;

/** Four minutes are enough of any song, however long. */
const ENOUGH_SECONDS = 240;

export function countsAsListened(position: number, length: number): boolean {
  if (length < TOO_SHORT_SECONDS) {
    return false;
  }
  return position >= Math.min(length / 2, ENOUGH_SECONDS);
}
