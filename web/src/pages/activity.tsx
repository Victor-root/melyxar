/*
 * What the server is doing, what it did, and what it still has to do.
 *
 * What is running is taken from the one place that watches it, rather than
 * asked for again here. This page used to ask on its own and start looking
 * again only once it had seen something running, so work started anywhere else
 * never appeared: the bar at the top said a scan was under way and this page,
 * two centimetres below it, said nothing was. Two answers to one question is
 * one answer too many, and the wrong one is always the one somebody reads.
 *
 * What finished is this page's own business, and is asked for again whenever
 * the work being watched comes to an end. So is the upkeep, which is the one
 * thing here that is not a job at all: it is the work nobody has started yet.
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError, REFRESH_MODES } from "../api";
import type { Job, Library, RefreshMode, Upkeep, UpkeepTask } from "../api";
import { JobLine } from "../components/job";
import { CopyReport } from "../components/report";
import { refusalKey } from "../i18n";
import { useRunning } from "../running";
import { useSettings } from "../settings";

export function ActivityPage({ libraries }: { libraries: Library[] }) {
  const { t } = useSettings();
  const { jobs: running, watch, finished } = useRunning();
  const [recent, setRecent] = useState<Job[]>([]);
  const [upkeep, setUpkeep] = useState<Upkeep | null>(null);
  const [failed, setFailed] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const [said, setSaid] = useState<string | null>(null);
  /* How much a scan started from here goes over. Kept for the page rather than
     asked once per button: somebody choosing "everything again" is choosing it
     for what they are about to press, and having to choose it twice is how the
     second library quietly gets the light one. */
  const [mode, setMode] = useState<RefreshMode>("what_is_missing");

  /* One library at a time, which is what this page is for: the bar at the top
     already offers the whole lot at once, and somebody who came here came to
     act on one thing. Whatever is started is watched at once rather than left
     to the slow beat, because a button that shows nothing for twenty seconds
     is indistinguishable from a button that did nothing. */
  const startOn = useCallback(
    async (asked: Promise<unknown>) => {
      setRefused(null);
      setSaid(null);
      try {
        await asked;
        watch();
      } catch (error) {
        setRefused(error instanceof ApiError ? error.code : "generic");
      }
    },
    [watch],
  );

  const loadRecent = useCallback(async (signal?: AbortSignal) => {
    try {
      setRecent((await api.jobs(signal)).recent);
      setFailed(false);
    } catch (error) {
      if (!(error instanceof DOMException)) {
        setFailed(true);
      }
    }
  }, []);

  const loadUpkeep = useCallback(async (signal?: AbortSignal) => {
    try {
      setUpkeep(await api.upkeep(signal));
    } catch (error) {
      if (!(error instanceof DOMException)) {
        setFailed(true);
      }
    }
  }, []);

  /* On the way in, and again each time the work being watched comes to an
     end: that is exactly when something has moved from running to finished,
     and when what the upkeep has left has just gone down. */
  useEffect(() => {
    const controller = new AbortController();
    loadRecent(controller.signal);
    loadUpkeep(controller.signal);
    return () => controller.abort();
  }, [loadRecent, loadUpkeep, finished]);

  /* Started from here, so this page says what happened rather than leaving a
     button that did nothing to look exactly like a button that is broken. */
  const startTheUpkeep = useCallback(async () => {
    setRefused(null);
    try {
      const { started } = await api.runUpkeep();
      setSaid(started > 0 ? t("upkeep.started", { count: started }) : t("upkeep.nothing_started"));
      watch();
      loadUpkeep();
    } catch (error) {
      setRefused(error instanceof ApiError ? error.code : "generic");
    }
  }, [loadUpkeep, t, watch]);

  const waiting = upkeep?.tasks.filter((task) => task.waiting > 0) ?? [];

  return (
    <main className="page">
      <div className="section-head">
        <h1>{t("jobs.title")}</h1>
        {/* Next to the work it describes: this is the page somebody lands on
            when something did not happen, and one button beats knowing which
            question to ask. */}
        <CopyReport />
      </div>

      <div className="controls">
        <label className="choice">
          <span className="choice-label">{t("refresh.mode")}</span>
          <select value={mode} onChange={(event) => setMode(event.target.value as RefreshMode)}>
            {REFRESH_MODES.map((one) => (
              <option key={one} value={one}>
                {t(`refresh.${one}`)}
              </option>
            ))}
          </select>
        </label>
      </div>
      {/* What the chosen mode costs, in one line. These three differ by hours
          of work, and a name alone does not say which. */}
      <p className="settings-why">{t(`refresh.${mode}_why`)}</p>

      <div className="controls">
        {libraries.map((library) => (
          <span key={library.id} className="control-group">
            <button className="button" onClick={() => startOn(api.scan(library.id, mode))}>
              {t("home.scan")} · {library.name}
            </button>
            <button className="button" onClick={() => startOn(api.identify(library.id, mode))}>
              {t("home.identify")}
            </button>
          </span>
        ))}
      </div>

      {/* A button that fails in silence is the same thing as a button that
          does nothing, and sends somebody to a terminal. */}
      {refused && <p className="notice">{t(refusalKey(refused))}</p>}
      {said && <p className="notice">{said}</p>}
      {failed && <p className="notice">{t("error.unreachable")}</p>}

      <section className="section">
        <h2>{t("jobs.running")}</h2>
        {running.length === 0 ? (
          <p className="notice notice-faint">{t("jobs.none")}</p>
        ) : (
          running.map((job) => (
            <JobLine
              key={job.id}
              job={job}
              onCancel={() => api.cancelJob(job.id).then(() => watch())}
            />
          ))
        )}
      </section>

      {upkeep && (
        <section className="section">
          <div className="section-head">
            <h2>{t("upkeep.title")}</h2>
            {/* Only when there is something to start: a button that would do
                nothing is a button somebody presses twice. */}
            {waiting.length > 0 && (
              <button className="button button-small" onClick={startTheUpkeep}>
                {t("upkeep.run_all")}
              </button>
            )}
          </div>
          <p className="settings-why">{t("upkeep.why")}</p>
          <p className="settings-why">
            {upkeep.next_run
              ? t("upkeep.next_run", { when: whenItIs(upkeep.next_run) })
              : t("upkeep.nightly_off")}
          </p>

          {upkeep.tasks.map((task) => (
            <div className="upkeep-line" key={`${task.library}-${task.task}`}>
              <span className="upkeep-what">
                <span className="upkeep-name">{t(`upkeep.${task.task}`)}</span>
                <span className="upkeep-library">{task.library_name}</span>
              </span>
              <span className="upkeep-count">
                {task.waiting > 0
                  ? t("upkeep.waiting", { count: task.waiting })
                  : t("upkeep.nothing_waiting")}
                {task.done > 0 && ` · ${t("upkeep.done_count", { count: task.done })}`}
              </span>
              {/* When it last ran, beside how much is left. A reading that has
                  never run and one that ran last night and found nothing look
                  alike from a count alone. */}
              <span className="upkeep-note">{lastRun(task, t)}</span>
              {task.during_the_scan && (
                <span className="upkeep-note">{t("upkeep.during_the_scan")}</span>
              )}
              {task.under_way ? (
                <span className="upkeep-note">{t("upkeep.under_way")}</span>
              ) : (
                task.waiting > 0 && (
                  <button
                    className="button button-small"
                    onClick={() =>
                      startOn(api.runUpkeepTask(task.library, task.task)).then(() => loadUpkeep())
                    }
                  >
                    {t("upkeep.run")}
                  </button>
                )
              )}
            </div>
          ))}
        </section>
      )}

      {recent.length > 0 && (
        <section className="section">
          <div className="section-head">
            <h2>{t("jobs.recent")}</h2>
            {/* Kept next to the list it empties, and only shown when there is
                something to empty. */}
            <button
              className="button button-small"
              onClick={() => api.forgetFinishedJobs().then(() => loadRecent())}
            >
              {t("jobs.forget")}
            </button>
          </div>
          {recent.map((job) => (
            <JobLine key={job.id} job={job} />
          ))}
        </section>
      )}
    </main>
  );
}

/*
 * An instant from the server, in the hour of whoever is reading it.
 *
 * The server keeps one clock and it is UTC, which is the only one it can read
 * with certainty. Three in the morning there is four here half the year, and
 * announcing the server's hour to somebody looking at their own clock is how a
 * run that happened on time looks like a run that did not.
 */
/*
 * When a reading last ran, in one short phrase.
 *
 * The time it took comes with it wherever it is known: a reading that took
 * four seconds and one that took four hours are two different answers to
 * "did the upkeep run last night", and only the second one explains a machine
 * that was busy all night.
 */
function lastRun(
  task: UpkeepTask,
  t: (key: string, values?: Record<string, string | number>) => string,
): string {
  if (!task.last_run) {
    return t("upkeep.never_run");
  }
  const when = whenItIs(task.last_run);
  return task.last_run_seconds === null
    ? t("upkeep.last_run_unknown", { when })
    : t("upkeep.last_run", { when, seconds: task.last_run_seconds });
}

function whenItIs(instant: string | null): string {
  if (!instant) {
    return "";
  }
  const when = new Date(instant);
  return Number.isNaN(when.getTime()) ? instant : when.toLocaleString();
}
