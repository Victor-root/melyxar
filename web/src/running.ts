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
 * Asked for often while something is running and rarely when nothing is. Never
 * stopping outright matters: work can be started from somewhere this interface
 * knows nothing about, another tab or a terminal, and an interface that says
 * nothing is running while a scan grinds away is an interface that lies.
 */

import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import { api } from "./api";
import { refusalOf, useTold } from "./asking";
import type { Job, Library } from "./api";

/** How often the server is asked while it is busy. */
const WHILE_BUSY_MS = 1000;

/** How long after something was started here the server is still asked at
 *  the fast beat, for work that has not begun by the first answer: a scan
 *  started by the server itself after a write is not yet running when the
 *  request that caused it returns. */
const GRACE_MS = 8000;

/**
 * How often it is asked when nothing is running.
 *
 * Rarely, because the answer is almost always the same, and never never: a
 * scan started from a terminal or from another tab has to turn up here without
 * anybody reloading the page.
 */
const WHEN_IDLE_MS = 20_000;

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
  const [busy, setBusy] = useState(true);
  const [finished, setFinished] = useState(0);
  const wasBusy = useRef(false);
  const graceUntil = useRef(0);
  /* The last work the server said it had done, so that one begun and ended
     between two looks, a quick scan or a few covers, still counts as ended:
     never seen running, it would otherwise never be seen at all. */
  const lastDone = useRef<string | null | undefined>(undefined);

  const look = useCallback(async (signal?: AbortSignal) => {
    try {
      const answer = await api.jobs(signal);
      setJobs(answer.running);
      setBusy(answer.running.length > 0 || Date.now() < graceUntil.current);
      const newest = answer.recent[0]?.id ?? null;
      const doneUnseen = lastDone.current !== undefined && newest !== lastDone.current;
      lastDone.current = newest;
      if (answer.running.length > 0) {
        wasBusy.current = true;
        return;
      }
      if (wasBusy.current || doneUnseen) {
        wasBusy.current = false;
        setFinished((count) => count + 1);
      }
    } catch {
      // A server that did not answer is not worth troubling a viewer with
      // here: the page it is on has its own way of saying so.
    }
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    look(controller.signal);
    const timer = window.setInterval(() => look(), busy ? WHILE_BUSY_MS : WHEN_IDLE_MS);
    return () => {
      window.clearInterval(timer);
      controller.abort();
    };
  }, [busy, look]);

  /* Asked for the moment something was started here, rather than waiting out
     the slow beat: a button that shows nothing for twenty seconds is
     indistinguishable from a button that did nothing. */
  const watch = useCallback(() => {
    graceUntil.current = Date.now() + GRACE_MS;
    setBusy(true);
    void look();
  }, [look]);

  return { jobs, watch, finished };
}

/** What the server is doing, for any page or part of the bar that shows it. */
export function useRunning(): Running {
  return useContext(RunningContext);
}

/** A button that asks the server to start something, and what became of it. */
export interface Starter {
  start: () => Promise<void>;
  starting: boolean;
  /** Why the server refused, as a code to be translated. A button that fails
      in silence is the same thing as a button that does nothing, and sends
      somebody to a terminal. */
  refused: string | null;
}

/** Asks the server to start one kind of work on every library. */
function useStartWork(
  libraries: Library[],
  ask: (library: string) => Promise<unknown>,
): Starter {
  const { watch } = useRunning();
  const told = useTold(async () => {
    await Promise.all(libraries.map((library) => ask(library.id)));
    watch();
  });

  return {
    start: told.tell,
    starting: told.busy,
    refused: told.failure && refusalOf(told.failure),
  };
}

/** How a scan is asked for, wherever the button lives. */
export function useStartScan(libraries: Library[]): Starter {
  return useStartWork(libraries, api.scan);
}

/** How a look up of what is still nameless is asked for. */
export function useStartIdentification(libraries: Library[]): Starter {
  return useStartWork(libraries, api.identify);
}
