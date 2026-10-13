/*
 * The mark of the song playing, inside its red button: four small bars
 * rising and falling each on a beat of their own, as a little equaliser
 * would, in place of the pause sign. The pause sign comes back under the
 * pointer or the keyboard, which is when it is about to be pressed. They
 * follow nothing of the song, and move by transform alone.
 */

import { PauseIcon } from "../player/icons";

/** How many bars stand in the button. */
const BARS = 4;

export function PlayingBars() {
  return (
    <>
      <span className="playing-bars" aria-hidden="true">
        {Array.from({ length: BARS }, (_, at) => (
          <span key={at} className="playing-bar" />
        ))}
      </span>
      <span className="playing-bars-pause">
        <PauseIcon size={20} />
      </span>
    </>
  );
}
