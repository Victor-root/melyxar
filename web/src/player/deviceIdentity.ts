/*
 * Naming this browser installation for its own calibration, and nothing else.
 *
 * Not an account, not a login, not a right to anything: a value made up once
 * and kept, so that what is measured on this machine is never handed to
 * another one that happens to sign in under the same name. Lost the moment
 * storage is cleared, which is the right answer: a machine that forgot who it
 * was calibrates again rather than borrowing a stranger's answer.
 */

const REMEMBERED = "melyxar.playback_client_id";

/** This device's own identifier, made up the first time it is asked for and
 *  kept from then on. */
export function deviceIdentity(): string {
  try {
    const kept = window.localStorage.getItem(REMEMBERED);
    if (kept) {
      return kept;
    }
    const made = crypto.randomUUID();
    window.localStorage.setItem(REMEMBERED, made);
    return made;
  } catch {
    // A browser refusing storage still calibrates for the length of this
    // sitting; it simply starts from nothing again next time.
    return crypto.randomUUID();
  }
}
