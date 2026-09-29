/*
 * Whether an administrator is to be taken through the first steps of this
 * server, and putting them behind it.
 *
 * Asked once, and only for an administrator: nobody else could take them.
 */

import { useCallback, useEffect, useState } from "react";
import { api } from "../api";

export interface FirstSteps {
  /** Still being asked, until the server has said. */
  pending: boolean | null;
  finish: () => Promise<void>;
}

export function useFirstSteps(isAdministrator: boolean): FirstSteps {
  const [pending, setPending] = useState<boolean | null>(isAdministrator ? null : false);

  useEffect(() => {
    if (!isAdministrator) {
      setPending(false);
      return;
    }
    const controller = new AbortController();
    api
      .firstSteps(controller.signal)
      .then((said) => setPending(said.pending))
      .catch(() => {
        // A server that will not say leaves the library open: everything the
        // first steps offer is in the administration too.
        if (!controller.signal.aborted) {
          setPending(false);
        }
      });
    return () => controller.abort();
  }, [isAdministrator]);

  const finish = useCallback(async () => {
    const said = await api.finishFirstSteps();
    setPending(said.pending);
  }, []);

  return { pending, finish };
}
