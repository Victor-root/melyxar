/*
 * What the activity screen is driven by, and nothing about how it looks.
 *
 * What is running is taken from the one place that watches it, rather than
 * asked for again here. This screen used to ask on its own and start looking
 * again only once it had seen something running, so work started anywhere else
 * never appeared: the bar at the top said a scan was under way and the screen
 * two centimetres below it said nothing was. Two answers to one question is one
 * answer too many, and the wrong one is always the one somebody reads.
 *
 * What finished is this screen's own business, and is asked for again whenever
 * the work being watched comes to an end. So is the upkeep, which is the one
 * thing here that is not a job at all: it is the work nobody has started yet.
 */

import { useCallback, useState } from "react";
import { api } from "../api";
import type { Job, RefreshMode, Upkeep, UpkeepTask } from "../api";
import { refusalOf, useAsked } from "../asking";
import { useRunning } from "../running";

/**
 * An instant from the server, in the hour of whoever is reading it.
 *
 * The server keeps one clock and it is UTC, which is the only one it can read
 * with certainty. Three in the morning there is four here half the year, and
 * announcing the server's hour to somebody looking at their own clock is how a
 * run that happened on time looks like a run that did not.
 */
export function whenItIs(instant: string | null): string {
  if (!instant) {
    return "";
  }
  const when = new Date(instant);
  return Number.isNaN(when.getTime()) ? instant : when.toLocaleString();
}

/** Everything the activity screen is handed to draw itself and be driven by. */
export interface ActivityScreen {
  /** What is running right now, from the one place that watches it. */
  running: Job[];
  /** What finished, newest first. */
  recent: Job[];
  /** What the upkeep still has to do, or nothing until the server has said. */
  upkeep: Upkeep | null;
  /** The readings with something left, which is what a button to start them
      all is offered for. */
  waiting: UpkeepTask[];
  /** Whether the server could not be asked. */
  failed: boolean;
  /** Why the server refused what was last started, as a code to be worded. */
  refused: string | null;
  /** How many readings the last press of "start the upkeep" set going, or
      nothing when it has not been pressed since. A count rather than a
      sentence: nought started and three started are two different answers,
      and wording them is the screen's business. */
  started: number | null;
  /** How much a scan started from here goes over. Held for the whole screen
      rather than asked once per button: somebody choosing "everything again"
      is choosing it for what they are about to press, and having to choose it
      twice is how the second library quietly gets the light one. */
  mode: RefreshMode;
  setMode: (mode: RefreshMode) => void;
  /** Starting one piece of work on one library, and watching it at once
      rather than leaving it to the slow beat. */
  startOn: (asked: Promise<unknown>) => Promise<void>;
  /** Starting every reading the upkeep has waiting. */
  startTheUpkeep: () => Promise<void>;
  /** Starting one reading of the upkeep on one library. */
  startOneReading: (task: UpkeepTask) => Promise<void>;
  /** Stopping one job. */
  cancel: (job: string) => void;
  /** Emptying the list of what finished. */
  forgetFinished: () => void;
}

export function useActivityScreen(): ActivityScreen {
  const { jobs: running, watch, finished } = useRunning();
  const [refused, setRefused] = useState<string | null>(null);
  const [started, setStarted] = useState<number | null>(null);
  const [mode, setMode] = useState<RefreshMode>("what_is_missing");

  /* On the way in, and again each time the work being watched comes to an
     end: that is exactly when something has moved from running to finished,
     and when what the upkeep has left has just gone down. */
  const jobs = useAsked((signal) => api.jobs(signal), [finished]);
  const upkeep = useAsked((signal) => api.upkeep(signal), [finished]);

  const startOn = useCallback(
    async (asked: Promise<unknown>) => {
      setRefused(null);
      setStarted(null);
      try {
        await asked;
        watch();
      } catch (error) {
        setRefused(refusalOf(error));
      }
    },
    [watch],
  );

  const askUpkeepAgain = upkeep.again;
  const startTheUpkeep = useCallback(async () => {
    setRefused(null);
    try {
      const { started: howMany } = await api.runUpkeep();
      setStarted(howMany);
      watch();
      askUpkeepAgain();
    } catch (error) {
      setRefused(refusalOf(error));
    }
  }, [askUpkeepAgain, watch]);

  const startOneReading = useCallback(
    async (task: UpkeepTask) => {
      await startOn(api.runUpkeepTask(task.library, task.task));
      askUpkeepAgain();
    },
    [askUpkeepAgain, startOn],
  );

  const cancel = useCallback(
    (job: string) => {
      void api.cancelJob(job).then(watch);
    },
    [watch],
  );

  const askJobsAgain = jobs.again;
  const forgetFinished = useCallback(() => {
    void api.forgetFinishedJobs().then(askJobsAgain);
  }, [askJobsAgain]);

  return {
    running,
    recent: jobs.answer?.recent ?? [],
    upkeep: upkeep.answer,
    waiting: upkeep.answer?.tasks.filter((task) => task.waiting > 0) ?? [],
    failed: jobs.failure !== null || upkeep.failure !== null,
    refused,
    started,
    mode,
    setMode,
    startOn,
    startTheUpkeep,
    startOneReading,
    cancel,
    forgetFinished,
  };
}
