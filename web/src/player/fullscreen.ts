/*
 * Whether the player is filling the screen, and how it is asked to.
 *
 * On its own because two places need it and neither is the owner of the other:
 * the button in the controls, and a double click on the picture itself. Held
 * in one place so that both do exactly the same thing, and so that the state
 * comes from the browser rather than from whoever pressed last: a viewer who
 * leaves fullscreen with the escape key never told either of them.
 */

import { useCallback, useEffect, useState } from "react";

export interface Fullscreen {
  /** Whether the screen is filled right now. */
  filling: boolean;
  /** Fill the screen, or leave it when it is already filled. */
  toggle: () => void;
}

/** What goes fullscreen is the picture and its controls together: an element
 *  sent on its own leaves every control behind on a page nobody can see. */
export function useFullscreen(stage: React.RefObject<HTMLDivElement | null>): Fullscreen {
  const [filling, setFilling] = useState(false);

  useEffect(() => {
    const tell = () => setFilling(document.fullscreenElement !== null);
    tell();
    document.addEventListener("fullscreenchange", tell);
    return () => document.removeEventListener("fullscreenchange", tell);
  }, []);

  const toggle = useCallback(() => {
    if (document.fullscreenElement) {
      void document.exitFullscreen();
    } else {
      void stage.current?.requestFullscreen?.();
    }
  }, [stage]);

  return { filling, toggle };
}
