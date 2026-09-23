/*
 * What the server is doing, what it did, and what it still has to do.
 *
 * What is running is taken from the one place that watches it, rather than
 * asked for again here: two answers to one question is one answer too many,
 * and the wrong one is always the one somebody reads. What finished and what
 * the upkeep has left are asked for again whenever the work being watched
 * comes to an end.
 */

import { api, REFRESH_MODES } from "../../api";
import type { Job, RefreshMode, UpkeepTask } from "../../api";
import { PageHead, Panel, Picker, Setting, Toggle } from "../../components/panel";
import { refusalKey } from "../../i18n";
import {
  ClockIcon,
  FolderIcon,
  HistoryIcon,
  KindIcon,
  PlayIcon,
  RefreshIcon,
  TasksIcon,
} from "../../icons";
import { useLibraries } from "../../libraries";
import { asLocalTime, asUtcMinutes, outOfAHundred, whenItIs } from "../../readable";
import type { Wording } from "../../readable";
import { useActivityScreen } from "../../screens/activity";
import { useLibraryWork } from "../../screens/settings";
import { useSettings } from "../../settings";

export function AdminTasks() {
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
  const { all: libraries } = useLibraries();
  const work = useLibraryWork();

  return (
    <>
      <PageHead lead={t("admin.tasks_page_lead")} />

      {/* A button that fails in silence is the same thing as a button that
          does nothing, and sends somebody to a terminal. */}
      {(refused || failed || started !== null) && (
        <p className={`panel-notice${refused || failed ? " panel-notice-trouble" : ""}`}>
          {refused
            ? t(refusalKey(refused))
            : failed
              ? t("error.unreachable")
              : started! > 0
                ? t("upkeep.started", { count: started! })
                : t("upkeep.nothing_started")}
        </p>
      )}

      <div className="panels">
        <Panel icon={TasksIcon} title={t("jobs.running")} lead={t("admin.running_lead")}>
          {running.length === 0 ? (
            <p className="empty-line">{t("jobs.none")}</p>
          ) : (
            running.map((job) => <JobCard key={job.id} job={job} onCancel={() => cancel(job.id)} />)
          )}
        </Panel>

        <Panel icon={FolderIcon} title={t("admin.start_work")} lead={t("admin.start_work_lead")}>
          <Setting label={t("refresh.mode")} why={t(`refresh.${mode}_why`)}>
            <Picker<RefreshMode>
              label={t("refresh.mode")}
              value={mode}
              options={REFRESH_MODES.map((one) => [one, t(`refresh.${one}`)] as const)}
              onPick={setMode}
            />
          </Setting>
          <div className="lines">
            {libraries.map((library) => (
              <div className="line" key={library.id}>
                <span className="line-mark" aria-hidden="true">
                  <KindIcon kind={library.kind} size={18} />
                </span>
                <span className="line-words">
                  <span className="line-name">{library.name}</span>
                </span>
                <span className="line-end">
                  <button
                    className="button button-small"
                    onClick={() => startOn(api.scan(library.id, mode))}
                  >
                    <RefreshIcon size={15} />
                    {t("home.scan")}
                  </button>
                  <button
                    className="button button-small"
                    onClick={() => startOn(api.identify(library.id, mode))}
                  >
                    {t("home.identify")}
                  </button>
                </span>
              </div>
            ))}
          </div>
        </Panel>
      </div>

      <Panel
        icon={ClockIcon}
        title={t("upkeep.title")}
        lead={t("upkeep.why")}
        action={
          waiting.length > 0 && (
            <button className="button button-small button-accent" onClick={startTheUpkeep}>
              <PlayIcon size={14} />
              {t("upkeep.run_all")}
            </button>
          )
        }
      >
        {work.kept && (
          <div className="settings-lines">
            <Setting label={t("settings.upkeep_nightly")} why={t("settings.upkeep_why")}>
              <Toggle
                label={t("settings.upkeep_nightly")}
                checked={work.kept.upkeep_nightly}
                onChange={(upkeep_nightly) => work.setTo({ upkeep_nightly })}
              />
            </Setting>
            {/* Chosen and shown in the time of this browser. The server keeps
                it in universal time, the only clock it can read with
                certainty, so the hour shown here shifts by one when the clocks
                change until somebody sets it again. */}
            <Setting label={t("admin.upkeep_hour")} why={t("settings.upkeep_at_why")}>
              <input
                type="time"
                className="field-line field-time"
                aria-label={t("admin.upkeep_hour")}
                value={asLocalTime(work.kept.upkeep_at_utc_minutes)}
                disabled={!work.kept.upkeep_nightly}
                onChange={(event) =>
                  work.setTo({ upkeep_at_utc_minutes: asUtcMinutes(event.target.value) })
                }
              />
            </Setting>
          </div>
        )}

        {upkeep && (
          <>
            <p className="panel-say">
              {upkeep.next_run
                ? t("upkeep.next_run", { when: whenItIs(upkeep.next_run) })
                : t("upkeep.nightly_off")}
            </p>
            <div className="lines">
              {upkeep.tasks.map((task) => (
                <UpkeepLine
                  key={`${task.library}-${task.task}`}
                  task={task}
                  onStart={() => startOneReading(task)}
                />
              ))}
            </div>
          </>
        )}
      </Panel>

      <Panel
        icon={HistoryIcon}
        title={t("jobs.recent")}
        lead={t("admin.recent_jobs_lead")}
        action={
          recent.length > 0 && (
            <button className="button button-small" onClick={forgetFinished}>
              {t("jobs.forget")}
            </button>
          )
        }
      >
        {recent.length === 0 ? (
          <p className="empty-line">{t("admin.no_recent_jobs")}</p>
        ) : (
          recent.map((job) => <JobCard key={job.id} job={job} />)
        )}
      </Panel>
    </>
  );
}

/** What a job's state is, as a colour. */
const STATE_OF: Record<Job["state"], "ok" | "attention" | "trouble" | "running"> = {
  queued: "attention",
  running: "running",
  succeeded: "ok",
  failed: "trouble",
  cancelled: "attention",
  interrupted: "attention",
};

/** One piece of work, running or finished. */
function JobCard({ job, onCancel }: { job: Job; onCancel?: () => void }) {
  const { t } = useSettings();
  const state = STATE_OF[job.state];

  return (
    <div className={`job-card${state === "running" ? " job-card-running" : ""}`}>
      <div className="job-card-head">
        <span className="job-card-name">{t(`jobs.${job.kind}`)}</span>
        {/* A scan is four passes end to end and the long ones are last, so a
            bar fills up, drops back to nothing and sets off again. Without a
            word saying which pass that is, it reads as a server that crashed
            and started over. */}
        {job.step && <span className="line-note">{t(`jobs.step.${job.step}`)}</span>}
        <span className={`state-pill state-${state === "running" ? "ok" : state}`}>
          <span className="state-dot" aria-hidden="true" />
          {t(`jobs.state.${job.state}`)}
        </span>
        {onCancel && (
          <button className="button button-small" onClick={onCancel}>
            {t("jobs.cancel")}
          </button>
        )}
      </div>

      {job.ratio !== null && (
        <div className="job-card-progress">
          <span className="meter">
            <span className="meter-fill" style={{ width: `${outOfAHundred(job.ratio)}%` }} />
          </span>
          <span className="job-card-count">
            {job.done} / {job.total} · {outOfAHundred(job.ratio)} %
          </span>
        </div>
      )}
      {job.ratio === null && job.done > 0 && <span className="job-card-count">{job.done}</span>}

      {/* Which file, right now, whole: a pass name and a bar do not tell a
          server that is working from one stuck on a four hour film. */}
      {job.doing && <span className="job-card-doing">{job.doing}</span>}
      {job.failure_reason && <span className="job-card-reason">{job.failure_reason}</span>}
    </div>
  );
}

/** One reading the upkeep does, on one library. */
function UpkeepLine({ task, onStart }: { task: UpkeepTask; onStart: () => void }) {
  const { t } = useSettings();
  return (
    <div className="line">
      <span className="line-words">
        <span className="line-name">
          {t(`upkeep.${task.task}`)} · {task.library_name}
        </span>
        {/* When it last ran, beside how much is left: a reading that never
            ran and one that ran last night and found nothing look alike from
            a count alone. */}
        <span className="line-note">
          {lastRun(task, t)}
          {task.during_the_scan && ` · ${t("upkeep.during_the_scan")}`}
        </span>
      </span>
      <span className="line-end">
        {/* Counted in whatever the reading is really done in: listening for
            the titles a season shares is done season by season. */}
        <span>
          {task.waiting > 0
            ? t(task.counts_seasons ? "upkeep.waiting_seasons" : "upkeep.waiting", {
                count: task.waiting,
              })
            : t("upkeep.nothing_waiting")}
        </span>
        {task.under_way ? (
          <span className="state-pill state-ok">
            <span className="state-dot" aria-hidden="true" />
            {t("upkeep.under_way")}
          </span>
        ) : (
          task.waiting > 0 && (
            <button className="button button-small" onClick={onStart}>
              {t("upkeep.run")}
            </button>
          )
        )}
      </span>
    </div>
  );
}

/*
 * When a reading last ran, in one short phrase, with the time it took: a
 * reading that took four seconds and one that took four hours are two
 * different answers to "did the upkeep run last night".
 */
function lastRun(task: UpkeepTask, t: Wording): string {
  if (!task.last_run) {
    return t("upkeep.never_run");
  }
  const when = whenItIs(task.last_run);
  return task.last_run_seconds === null
    ? t("upkeep.last_run_unknown", { when })
    : t("upkeep.last_run", { when, seconds: task.last_run_seconds });
}
