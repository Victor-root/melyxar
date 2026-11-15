/*
 * Whether the player is filling the screen, and how it is asked to.
 *
 * On its own because two places need it and neither is the owner of the other:
 * the button in the controls, and a double click on the picture itself. Held
 * in one place so that both do exactly the same thing, and so that the state
 * comes from the browser rather than from whoever pressed last: a viewer who
 * leaves fullscreen with the escape key never told either of them.
 *
 * For a moment after the screen is filled or left, the stage says it is
 * settling, and the picture is drawn a hair short of opaque (player.css). On
 * the maintainer's Windows laptop, Brave now and then kept showing the last
 * picture at its former size after leaving fullscreen, squashed for a second,
 * and for as long as the film stayed paused: the picture had been handed to
 * the graphics chip on its own, outside the page, and stayed there at the old
 * size until another came. A picture not quite opaque is never handed over,
 * so it is drawn with the page, at the size the page gives it. The page's own
 * sizes were right all along (the journal's "around a change to or from the
 * whole screen").
 */

import { useCallback, useEffect, useState } from "react";
import { appPlayer } from "./app-player";

export interface Fullscreen {
  /** Whether the screen is filled right now. */
  filling: boolean;
  /** Fill the screen, or leave it when it is already filled. */
  toggle: () => void;
}

/** How long the picture stays drawn with the page after the screen is filled
 *  or left: longer than the second it was seen squashed for. */
const SETTLING_MS = 1500;

/** What goes fullscreen is the picture and its controls together: an element
 *  sent on its own leaves every control behind on a page nobody can see. */
export function useFullscreen(stage: React.RefObject<HTMLDivElement | null>): Fullscreen {
  const [filling, setFilling] = useState(false);

  useEffect(() => {
    let settled: number | undefined;
    const tell = () => setFilling(document.fullscreenElement !== null);
    const changed = () => {
      tell();
      const settling = stage.current;
      if (!settling) {
        return;
      }
      settling.dataset.settling = "";
      window.clearTimeout(settled);
      settled = window.setTimeout(() => {
        delete settling.dataset.settling;
      }, SETTLING_MS);
    };
    tell();
    document.addEventListener("fullscreenchange", changed);
    return () => {
      document.removeEventListener("fullscreenchange", changed);
      window.clearTimeout(settled);
    };
  }, [stage]);

  const toggle = useCallback(() => {
    // Inside the application, the page already fills its window: what is
    // left to fill is the room of the phone's own bars, which only the
    // application can take.
    if (appPlayer) {
      appPlayer.fillScreen(!filling);
      setFilling(!filling);
      return;
    }
    if (document.fullscreenElement) {
      void document.exitFullscreen();
    } else {
      void stage.current?.requestFullscreen?.();
    }
  }, [stage, filling]);

  return { filling, toggle };
}
