/*
 * The mark of the song playing, inside its red button: four small bars, as a
 * little equaliser would stand, in place of the pause sign, which comes back
 * under the pointer or the keyboard. The same bars stand in a corner of the
 * cover of the album or the playlist playing.
 *
 * While the wave behind the player is drawn, the bars rise and fall with it,
 * coarsely, from the same levels and nothing more; when there is no wave, the
 * account having turned it off or the song having no levels yet, they rise
 * and fall on beats of their own. Either way they move by transform alone,
 * and nothing of React is drawn again for them.
 */

import { useEffect, useRef } from "react";
import { PauseIcon } from "../player/icons";
import { hearWaveLevels } from "./player/wave-levels";

/** How long the bars wait for the wave before they go back to their own
 *  beats: a few of its drawings. */
const WAVE_GONE_MS = 300;

/** `withPause` puts the pause sign in their place under the pointer, for a
 *  button that pauses. */
export function PlayingBars({ withPause = true }: { withPause?: boolean }) {
  const box = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    const element = box.current;
    if (!element || window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      return;
    }
    const bars = Array.from(element.children) as HTMLElement[];
    let gone = 0;
    const leaveToTheirOwn = () => {
      delete element.dataset.heard;
      for (const bar of bars) {
        bar.style.transform = "";
      }
    };
    const stop = hearWaveLevels((heights) => {
      element.dataset.heard = "yes";
      bars.forEach((bar, at) => {
        bar.style.transform = `scaleY(${heights[at]})`;
      });
      window.clearTimeout(gone);
      gone = window.setTimeout(leaveToTheirOwn, WAVE_GONE_MS);
    });
    return () => {
      stop();
      window.clearTimeout(gone);
    };
  }, []);

  return (
    <>
      <span className="playing-bars" ref={box} aria-hidden="true">
        <span className="playing-bar" />
        <span className="playing-bar" />
        <span className="playing-bar" />
        <span className="playing-bar" />
      </span>
      {withPause && (
        <span className="playing-bars-pause">
          <PauseIcon size={20} />
        </span>
      )}
    </>
  );
}
