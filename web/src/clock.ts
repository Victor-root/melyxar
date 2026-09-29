/*
 * A moment of a film or a song, as somebody reads it.
 *
 * On its own so that the bar, the preview, the chapter cards and the lists of
 * songs all write a time the same way, and so that none of them has to reach
 * into another: music, kept apart from the player of films, reads it here.
 */

export function asClock(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) {
    return "0:00";
  }
  const whole = Math.floor(seconds);
  const hours = Math.floor(whole / 3600);
  const minutes = Math.floor((whole % 3600) / 60);
  const rest = whole % 60;
  const padded = `${minutes < 10 && hours > 0 ? "0" : ""}${minutes}:${rest < 10 ? "0" : ""}${rest}`;
  return hours > 0 ? `${hours}:${padded}` : padded;
}

/** A moment typed by hand, read back: "1:02", "1:02:03", or plain seconds.
 *  Nothing for anything else, which a field then leaves as it was. */
export function fromClock(text: string): number | null {
  const parts = text.trim().split(":");
  if (parts.length > 3 || parts.some((part) => !/^\d+(\.\d+)?$/.test(part))) {
    return null;
  }
  const numbers = parts.map(Number);
  // Only the first part may run past sixty: "90" is a minute and a half,
  // "1:90" is a slip.
  if (numbers.slice(1).some((part) => part >= 60)) {
    return null;
  }
  return numbers.reduce((total, part) => total * 60 + part, 0);
}
