/*
 * What the server is busy with, for any page that wants to show it.
 *
 * A scan takes minutes on a real library. Starting one and seeing nothing
 * happen is the same thing as a broken button: somebody presses it again, then
 * goes looking in a terminal. So whoever starts work watches it, and the page
 * fills itself in when it is done rather than waiting to be reloaded.
 *
 * Asked for again only while something is actually running. A page with
 * nothing to watch asks once and then leaves the server alone, which is the
 * same rule the activity page already follows.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "./api";
import type { Job } from "./api";

/** How often the server is asked while it is busy. */
const WHILE_BUSY_MS = 1500;

export interface Running {
  /** What is running right now, newest first as the server lists it. */
  jobs: Job[];
  /** Told to look again now, after starting something from this page. */
  watch: () => void;
}

/**
 * Watches what the server is doing, and says when it has finished.
 *
 * `onFinished` is called once each time the last running job ends, which is
 * what a page uses to fetch what the work produced.
 */
export function useRunning(onFinished?: () => void): Running {
  const [jobs, setJobs] = useState<Job[]>([]);
  const [watching, setWatching] = useState(true);
  /* Kept in a ref so that a page passing a fresh function on every render
     does not restart the watching each time. */
  const finished = useRef(onFinished);
  finished.current = onFinished;
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
        finished.current?.();
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

  return { jobs, watch };
}
