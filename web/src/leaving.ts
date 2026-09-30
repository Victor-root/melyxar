/*
 * Leaving a page for the one it was reached from.
 *
 * A page that finishes its job, a deleted list or a saved form, goes back to
 * where it came from by stepping back in the history. Sending it to that
 * address instead, even as a replacement, leaves the page it came from in the
 * history twice, and the back key has to be pressed twice to get out of it.
 * Only a page opened straight from an address, with nothing of the app behind
 * it, has no step back to take and goes to the address given.
 */

import { useCallback } from "react";
import { useLocation, useNavigate } from "react-router-dom";

export function useLeave(fallback: string): () => void {
  const navigate = useNavigate();
  const location = useLocation();
  const cameFromTheApp = location.key !== "default";
  return useCallback(() => {
    if (cameFromTheApp) {
      navigate(-1);
    } else {
      navigate(fallback, { replace: true });
    }
  }, [cameFromTheApp, fallback, navigate]);
}
