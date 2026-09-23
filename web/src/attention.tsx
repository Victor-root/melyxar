/*
 * What deserves an administrator's look, held once for the whole interface.
 *
 * The bell of the bar and the panel of the summary show the same points, and
 * marking them as seen in one quiets both at once: held here, beside the
 * rest of what is shared, rather than asked for twice and out of step. Asked
 * again every thirty seconds and whenever the page comes back into view.
 * Nobody but an administrator asks at all.
 */

import { createContext, useCallback, useContext, useEffect, useState } from "react";
import type { ReactNode } from "react";
import { useAccount } from "./account";
import { api } from "./api";
import type { AttentionPoint } from "./api";

/** How often the points are asked for again. */
const LOOKED_AT_EVERY_MS = 30_000;

export interface Attention {
  /** Nothing until the first answer, and for anybody not an administrator. */
  points: AttentionPoint[] | null;
  /** Quiets every point that can be, everywhere it is shown. */
  markSeen: () => Promise<void>;
}

const AttentionContext = createContext<Attention>({
  points: null,
  markSeen: async () => {},
});

export function useAttention(): Attention {
  return useContext(AttentionContext);
}

export function AttentionProvider({ children }: { children: ReactNode }) {
  const { account } = useAccount();
  const administrator = account?.is_administrator === true;
  const [points, setPoints] = useState<AttentionPoint[] | null>(null);

  const look = useCallback(() => {
    api
      .attention()
      .then((answer) => setPoints(answer.points))
      .catch(() => {
        // The last points stay: a question that failed once says nothing
        // about whether they are still true.
      });
  }, []);

  useEffect(() => {
    if (!administrator) {
      setPoints(null);
      return;
    }
    look();
    const timer = window.setInterval(look, LOOKED_AT_EVERY_MS);
    const onVisible = () => {
      if (document.visibilityState === "visible") {
        look();
      }
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [administrator, look]);

  const markSeen = useCallback(async () => {
    const answer = await api.markAttentionSeen();
    setPoints(answer.points);
  }, []);

  return (
    <AttentionContext.Provider value={{ points, markSeen }}>{children}</AttentionContext.Provider>
  );
}
