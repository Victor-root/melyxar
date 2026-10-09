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
 * the work being watched comes to an end. So are the scheduled tasks, which
 * are the one thing here that is not a job at all: they are what runs over
 * every library, and a run of one is several jobs.
 */

import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import type { Job, RefreshMode, ScheduledTasks, TaskName } from "../api";
import { refusalOf, useAsked } from "../asking";
import { useRunning } from "../running";
import { lookWhileSeen } from "../polling";

/** Everything the activity screen is handed to draw itself and be driven by. */
export interface ActivityScreen {
  /** What is running right now, from the one place that watches it. */
  running: Job[];
  /** What finished, newest first. */
  recent: Job[];
  /** The scheduled tasks, or nothing until the server has said. */
  tasks: ScheduledTasks | null;
  /** Whether the server could not be asked. */
  failed: boolean;
  /** Why the server refused what was last started, as a code to be worded. */
  refused: string | null;
  /** Whether the last press set every task going, so the screen says so. */
  started: boolean;
  /** How much a scan started from here goes over. Held for the whole screen
      rather than asked once per button: somebody choosing "everything again"
      is choosing it for what they are about to press, and having to choose it
      twice is how the second library quietly gets the light one. */
  mode: RefreshMode;
  setMode: (mode: RefreshMode) => void;
  /** Starting one piece of work on one library, and watching it at once
      rather than leaving it to the slow beat. */
  startOn: (asked: Promise<unknown>) => Promise<void>;
  /** Starting every task, one after the other. */
  startEveryTask: () => Promise<void>;
  /** Starting one task over every library. */
  startTask: (task: TaskName) => Promise<void>;
  /** Saying whether a task runs by itself every day, and when. */
  schedule: (task: TaskName, runs_on_schedule: boolean, at_utc_minutes: number) => void;
  /** Stopping one job. */
  cancel: (job: string) => void;
  /** Emptying the list of what finished. */
  forgetFinished: () => void;
}

export function useActivityScreen(): ActivityScreen {
  const { jobs: running, watch, finished } = useRunning();
  const [refused, setRefused] = useState<string | null>(null);
  const [started, setStarted] = useState(false);
  const [mode, setMode] = useState<RefreshMode>("what_is_missing");

  /* On the way in, and again each time the work being watched comes to an
     end: that is exactly when something has moved from running to finished,
     and when what the tasks have left has just gone down. */
  const jobs = useAsked((signal) => api.jobs(signal), [finished]);
  const asked = useAsked((signal) => api.tasks(signal), [finished]);
  /* What is shown, which a change made here writes into at once, before the
     server has answered, and which the server's answer then replaces. */
  const [tasks, setTasks] = useState<ScheduledTasks | null>(null);
  const answer = asked.answer;
  useEffect(() => {
    if (answer) {
      setTasks(answer);
    }
  }, [answer]);

  /* A task ends a moment after its last job does, once it has written down
     how the whole run went. Looked at again on a short beat while one is
     under way, so its line never stays running after it has ended. */
  const lookAgain = asked.look;
  const busy = tasks?.tasks.some((task) => task.under_way) ?? false;
  useEffect(() => {
    if (!busy) {
      return;
    }
    return lookWhileSeen(lookAgain, 3_000);
  }, [busy, lookAgain]);

  const startOn = useCallback(
    async (asking: Promise<unknown>) => {
      setRefused(null);
      setStarted(false);
      try {
        await asking;
        watch();
      } catch (error) {
        setRefused(refusalOf(error));
      }
    },
    [watch],
  );

  const askTasksAgain = asked.again;
  const startEveryTask = useCallback(async () => {
    setRefused(null);
    try {
      await api.runEveryTask();
      setStarted(true);
      watch();
      askTasksAgain();
    } catch (error) {
      setRefused(refusalOf(error));
    }
  }, [askTasksAgain, watch]);

  const startTask = useCallback(
    async (task: TaskName) => {
      setTasks((before) =>
        before && {
          ...before,
          tasks: before.tasks.map((one) => (one.task === task ? { ...one, under_way: true } : one)),
        },
      );
      await startOn(api.runTask(task));
      askTasksAgain();
    },
    [askTasksAgain, startOn],
  );

  const schedule = useCallback(
    (task: TaskName, runs_on_schedule: boolean, at_utc_minutes: number) => {
      setRefused(null);
      const before = tasks;
      setTasks(
        (now) =>
          now && {
            ...now,
            tasks: now.tasks.map((one) =>
              one.task === task ? { ...one, runs_on_schedule, at_utc_minutes } : one,
            ),
          },
      );
      api
        .scheduleTask(task, runs_on_schedule, at_utc_minutes)
        .then(setTasks)
        .catch((error) => {
          setTasks(before);
          setRefused(refusalOf(error));
        });
    },
    [tasks],
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
    tasks,
    failed: jobs.failure !== null || asked.failure !== null,
    refused,
    started,
    mode,
    setMode,
    startOn,
    startEveryTask,
    startTask,
    schedule,
    cancel,
    forgetFinished,
  };
}
