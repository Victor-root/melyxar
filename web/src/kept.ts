/*
 * What each screen last showed, kept for when it is shown again.
 *
 * A page walked back to used to ask the server everything again before it
 * could draw anything: a flash of an empty page, then its rows arriving one
 * after another, then the page jumping to where it had been left. What it
 * showed is kept here instead, so it is drawn again whole the moment it is
 * returned to, and the server is asked all the same, quietly, to bring it up
 * to date.
 *
 * Held in memory for the tab, a few dozen screens at most, and forgotten the
 * moment the account changes: what one account was shown is not another's.
 */

/** How many screens are kept: a long evening of going back and forth. */
const SCREENS_KEPT = 40;

const answers = new Map<string, unknown>();

/** Keeps what a screen was answered, the most recent last. */
export function keep(key: string, value: unknown) {
  answers.delete(key);
  answers.set(key, value);
  while (answers.size > SCREENS_KEPT) {
    const oldest = answers.keys().next().value;
    if (oldest === undefined) {
      break;
    }
    answers.delete(oldest);
  }
}

/** What a screen was last answered, when it was. Wrapped, so an answer that
 *  was itself nothing is told apart from no answer at all. */
export function recall<T>(key: string): { value: T } | undefined {
  return answers.has(key) ? { value: answers.get(key) as T } : undefined;
}

/** Forgets everything kept, for an account that leaves or arrives. */
export function forgetKept() {
  answers.clear();
}
