/*
 * What the server is busy with, watched once for the whole interface.
 *
 * A scan takes minutes on a real library. Starting one and seeing nothing
 * happen is the same thing as a broken button: somebody presses it again, then
 * goes looking in a terminal. So whoever starts work watches it, and the pages
 * fill themselves in when it is done rather than waiting to be reloaded.
 *
 * Watched in one place rather than per page: the bar at the top offers the
 * work and every page shows it, and two of them asking the same question twice
 * a second is one too many.
 *
 * Asked for again only while something is actually running. With nothing to
 * watch it asks once and then leaves the server alone.
 */

import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import { api } from "./api";
import type { Job, Library } from "./api";

/** How often the server is asked while it is busy. */
const WHILE_BUSY_MS = 1500;

export interface Running {
  /** What is running right now, newest first as the server lists it. */
  jobs: Job[];
  /** Told to look again now, after starting something. */
  watch: () => void;
  /** Counted up each time the last running job ends, so that a page which
      wants what the work produced asks for it again and nothing else does. */
  finished: number;
}

export const RunningContext = createContext<Running>({
  jobs: [],
  watch: () => {},
  finished: 0,
});

/** What the provider at the top of the interface holds. */
export function useWatchedWork(): Running {
  const [jobs, setJobs] = useState<Job[]>([]);
  const [watching, setWatching] = useState(true);
  const [finished, setFinished] = useState(0);
  const wasBusy = useRef(false);

  const look = useCallback(async (signal?: AbortSignal) => {
    try {
      const answer = await api.jobs(signal);
      setJobs(answer.running);
      if (answer.running.length > 0) {
        wasBusy.current = true;
        return;
      }
      if (wasBusy.current) {
        wasBusy.current = false;
        setFinished((count) => count + 1);
      }
      // Nothing is running, so there is nothing to come back for until
      // somebody starts something.
      setWatching(false);
    } catch {
      // A server that did not answer is not worth troubling a viewer with
      // here: the page it is on has its own way of saying so.
    }
  }, []);

  useEffect(() => {
    if (!watching) {
      return;
    }
    const controller = new AbortController();
    look(controller.signal);
    const timer = window.setInterval(() => look(), WHILE_BUSY_MS);
    return () => {
      window.clearInterval(timer);
      controller.abort();
    };
  }, [watching, look]);

  const watch = useCallback(() => {
    wasBusy.current = true;
    setWatching(true);
  }, []);

  return { jobs, watch, finished };
}

/** What the server is doing, for any page or part of the bar that shows it. */
export function useRunning(): Running {
  return useContext(RunningContext);
}

/** How a scan of every library is asked for, wherever the button lives. */
export function useStartScan(libraries: Library[]): {
  start: () => Promise<void>;
  starting: boolean;
} {
  const { watch } = useRunning();
  const [starting, setStarting] = useState(false);

  const start = useCallback(async () => {
    setStarting(true);
    try {
      await Promise.all(libraries.map((library) => api.scan(library.id)));
      watch();
    } finally {
      setStarting(false);
    }
  }, [libraries, watch]);

  return { start, starting };
}
