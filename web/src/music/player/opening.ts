/*
 * Whether the page of what is playing is open, kept in the history.
 *
 * It is a page laid over whatever the viewer was on, and it used to be a flag
 * of its own. The back button of a browser, and the back key of a phone, which
 * is the same thing, then went back behind it and left it standing, the cover
 * of the album still on the screen and the history a step further each time.
 * So opening it is a step of the history, on the same address, and anything
 * that goes back closes it.
 */

import { useCallback } from "react";
import { useLocation, useNavigate } from "react-router-dom";

/** The mark the step of the history carries while the page is open. */
interface Marked {
  nowPlaying?: boolean;
}

export function useNowPlayingPage() {
  const location = useLocation();
  const navigate = useNavigate();
  const marked = (location.state as Marked | null)?.nowPlaying === true;

  const open = useCallback(
    () => navigate({ pathname: location.pathname, search: location.search }, { state: { nowPlaying: true } satisfies Marked }),
    [navigate, location.pathname, location.search],
  );
  /* Back, since the step it was opened with is the one behind. */
  const close = useCallback(() => navigate(-1), [navigate]);
  /* Takes the mark off the step where it stands, for a page left open over
     nothing: nothing is playing any more, or the tab was loaded again. */
  const forget = useCallback(
    () => navigate({ pathname: location.pathname, search: location.search }, { replace: true }),
    [navigate, location.pathname, location.search],
  );
  return { marked, open, close, forget };
}
