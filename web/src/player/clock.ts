/*
 * A moment of a film, as somebody reads it.
 *
 * On its own so that the bar, the preview and the chapter cards all write a
 * time the same way, and so that neither of the two files that need it has to
 * reach into the other.
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
