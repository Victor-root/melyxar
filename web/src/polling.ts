/*
 * Asking again on a beat, for as long as somebody can see the page.
 *
 * A page left in the background kept asking the server every second or every
 * few seconds, for answers nobody was looking at. Here the beat goes on but
 * says nothing while the page is hidden, and asks once the moment it comes
 * back, so what is on screen is as fresh as it ever was when it is looked at.
 */

/** Runs `look` every `every` milliseconds while the page is shown, and once
 *  whenever it comes back into view. Answers how to stop. */
export function lookWhileSeen(look: () => void, every: number): () => void {
  const beat = () => {
    if (!document.hidden) {
      look();
    }
  };
  const timer = window.setInterval(beat, every);
  document.addEventListener("visibilitychange", beat);
  return () => {
    window.clearInterval(timer);
    document.removeEventListener("visibilitychange", beat);
  };
}
