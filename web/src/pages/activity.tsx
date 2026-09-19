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

import { api, REFRESH_MODES } from "../api";
import type { Library, RefreshMode, UpkeepTask } from "../api";
import { JobLine } from "../components/job";
import { CopyReport } from "../components/report";
import { refusalKey } from "../i18n";
import { whenItIs } from "../readable";
import { useActivityScreen } from "../screens/activity";
import { useSettings } from "../settings";

export function ActivityPage({ libraries }: { libraries: Library[] }) {
  const { t } = useSettings();
  const {
    running,
    recent,
    upkeep,
    waiting,
    failed,
    refused,
    started,
    mode,
    setMode,
    startOn,
    startTheUpkeep,
    startOneReading,
    cancel,
    forgetFinished,
  } = useActivityScreen();

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
              {t("home.identify")} · {library.name}
            </button>
          </span>
        ))}
      </div>

      {/* A button that fails in silence is the same thing as a button that
          does nothing, and sends somebody to a terminal. */}
      {refused && <p className="notice">{t(refusalKey(refused))}</p>}
      {started !== null && (
        <p className="notice">
          {started > 0 ? t("upkeep.started", { count: started }) : t("upkeep.nothing_started")}
        </p>
      )}
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
              onCancel={() => cancel(job.id)}
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
              {/* Counted in whatever the reading is really done in. Listening
                  for the titles a season shares is done season by season, so a
                  number of files there would be a number nobody can act on. */}
              <span className="upkeep-count">
                {task.waiting > 0
                  ? t(task.counts_seasons ? "upkeep.waiting_seasons" : "upkeep.waiting", {
                      count: task.waiting,
                    })
                  : t("upkeep.nothing_waiting")}
                {task.done > 0 &&
                  ` · ${t(task.counts_seasons ? "upkeep.done_seasons" : "upkeep.done_count", {
                    count: task.done,
                  })}`}
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
                    onClick={() => startOneReading(task)}
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
              onClick={forgetFinished}
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
