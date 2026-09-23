/*
 * What deserves an administrator's look, held once for the whole interface.
 *
 * The bell of the bar and the panel of the summary show the same points, and
 * marking them as seen in one quiets both at once: held here, beside the
 * rest of what is shared, rather than asked for twice and out of step. Asked
 * again the moment a line of the journal is written, since refusals and
 * failed tasks are counted from it, whenever the page comes back into view,
 * and every thirty seconds for what changes without a line, such as a disk
 * filling. Nobody but an administrator asks at all.
 */

import { createContext, useCallback, useContext, useEffect, useState } from "react";
import type { ReactNode } from "react";
import { useAccount } from "./account";
import { api } from "./api";
import type { AttentionPoint } from "./api";
import { useJournalNews } from "./live";

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
    if (!administrator) {
      return;
    }
    api
      .attention()
      .then((answer) => setPoints(answer.points))
      .catch(() => {
        // The last points stay: a question that failed once says nothing
        // about whether they are still true.
      });
  }, [administrator]);
  // A refusal or a failed task counted the moment it is written, and
  // everything looked at again whenever the line opens, the page having
  // come back into view.
  useJournalNews(look);

  useEffect(() => {
    if (!administrator) {
      setPoints(null);
      return;
    }
    const timer = window.setInterval(look, LOOKED_AT_EVERY_MS);
    return () => window.clearInterval(timer);
  }, [administrator, look]);

  const markSeen = useCallback(async () => {
    const answer = await api.markAttentionSeen();
    setPoints(answer.points);
  }, []);

  return (
    <AttentionContext.Provider value={{ points, markSeen }}>{children}</AttentionContext.Provider>
  );
}
