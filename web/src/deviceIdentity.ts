/*
 * Naming this browser installation, for its calibration and its session.
 *
 * Not an account, not a login, not a right to anything: a value made up once
 * and kept. What is measured on this machine is never handed to another one
 * that happens to sign in under the same name, and signing in again here
 * replaces the session this account already held here rather than adding a
 * second one. Lost the moment storage is cleared, which is the right answer:
 * a machine that forgot who it was calibrates again rather than borrowing a
 * stranger's answer, and signs in as a new device.
 */

const REMEMBERED = "melyxar.playback_client_id";

/**
 * A fresh random identifier, in the shape of a UUID.
 *
 * Not `crypto.randomUUID()`: that call is refused outside a secure context,
 * and this server is reached in plain HTTP on the local network, which one
 * is not. `crypto.getRandomValues` carries no such restriction and is all a
 * random identifier actually needs.
 */
function randomId(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

/** This device's own identifier, made up the first time it is asked for and
 *  kept from then on. */
export function deviceIdentity(): string {
  try {
    const kept = window.localStorage.getItem(REMEMBERED);
    if (kept) {
      return kept;
    }
    const made = randomId();
    window.localStorage.setItem(REMEMBERED, made);
    return made;
  } catch {
    // A browser refusing storage still calibrates for the length of this
    // sitting; it simply starts from nothing again next time.
    return randomId();
  }
}
