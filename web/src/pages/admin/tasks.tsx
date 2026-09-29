/*
 * What the server is doing, what it did, and what it still has to do.
 *
 * What is running is taken from the one place that watches it, rather than
 * asked for again here: two answers to one question is one answer too many,
 * and the wrong one is always the one somebody reads. What finished and what
 * the scheduled tasks have left are asked for again whenever the work being
 * watched comes to an end.
 */

import { api, REFRESH_MODES } from "../../api";
import type { Job, RefreshMode, ScheduledTask } from "../../api";
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
import type { ActivityScreen } from "../../screens/activity";
import { useSettings } from "../../settings";

export function AdminTasks() {
  const { t } = useSettings();
  const {
    running,
    recent,
    tasks,
    failed,
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
  } = useActivityScreen();
  const { all: libraries } = useLibraries();

  return (
    <>
      <PageHead lead={t("admin.tasks_page_lead")} />

      {/* A button that fails in silence is the same thing as a button that
          does nothing, and sends somebody to a terminal. */}
      {(refused || failed || started) && (
        <p className={`panel-notice${refused || failed ? " panel-notice-trouble" : ""}`}>
          {refused ? t(refusalKey(refused)) : failed ? t("error.unreachable") : t("tasks.started")}
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

      <ScheduledTasksPanel
        tasks={tasks}
        onStartEvery={startEveryTask}
        onStart={startTask}
        onSchedule={schedule}
      />

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

/** The tasks that run by themselves, each with the hour it runs at. The
 *  administration and the first steps of a new server both show it. */
export function ScheduledTasksPanel({
  tasks,
  onStartEvery,
  onStart,
  onSchedule,
}: {
  tasks: ActivityScreen["tasks"];
  onStartEvery: () => void;
  onStart: (task: ScheduledTask["task"]) => void;
  onSchedule: ActivityScreen["schedule"];
}) {
  const { t } = useSettings();
  return (
    <Panel
      icon={ClockIcon}
      title={t("tasks.title")}
      lead={t("tasks.why")}
      action={
        <button className="button button-small button-accent" onClick={onStartEvery}>
          <PlayIcon size={14} />
          {t("tasks.run_all")}
        </button>
      }
    >
      {tasks && (
        <>
          <p className="panel-say">
            {tasks.next_run
              ? t("tasks.next_run", { when: whenItIs(tasks.next_run) })
              : t("tasks.none_scheduled")}
          </p>
          <div className="task-lines">
            {tasks.tasks.map((task) => (
              <TaskLine
                key={task.task}
                task={task}
                onStart={() => onStart(task.task)}
                onSchedule={(runs, at) => onSchedule(task.task, runs, at)}
              />
            ))}
          </div>
        </>
      )}
    </Panel>
  );
}

/** One scheduled task: what it does, when it runs by itself, what it has
 *  waiting, and a button to run it now. */
function TaskLine({
  task,
  onStart,
  onSchedule,
}: {
  task: ScheduledTask;
  onStart: () => void;
  onSchedule: (runs_on_schedule: boolean, at_utc_minutes: number) => void;
}) {
  const { t } = useSettings();
  const name = t(`task.${task.task}`);
  return (
    <div className="task-line">
      <div className="task-line-words">
        <span className="line-name">{name}</span>
        {/* What it does, in words anybody can follow: seven lines nobody
            understands are seven switches nobody dares touch. */}
        <span className="task-line-why">{t(`task.${task.task}_why`)}</span>
        {/* When it last ran, beside how much is left: a task that never ran
            and one that ran last night and found nothing look alike from a
            count alone. */}
        <span className="line-note">{lastRun(task, t)}</span>
      </div>

      <div className="task-line-controls">
        {/* Chosen and shown in the time of this browser. The server keeps it
            in universal time, the only clock it can read with certainty, so
            the hour shown here shifts by one when the clocks change until
            somebody sets it again. */}
        <div className="task-line-when">
          <Toggle
            label={`${name} · ${t("tasks.daily")}`}
            checked={task.runs_on_schedule}
            onChange={(runs) => onSchedule(runs, task.at_utc_minutes)}
          />
          <span>{t("tasks.daily")}</span>
          <input
            type="time"
            className="field-line field-time"
            aria-label={`${name} · ${t("tasks.daily")}`}
            value={asLocalTime(task.at_utc_minutes)}
            disabled={!task.runs_on_schedule}
            onChange={(event) =>
              event.target.value && onSchedule(true, asUtcMinutes(event.target.value))
            }
          />
        </div>
        <div className="task-line-now">
          {/* Counted in whatever the task really works in: listening for the
              titles a season shares is done season by season. Nothing for
              the scan, which cannot know what a disk holds until it looks. */}
          {task.waiting !== null && (
            <span className="task-line-waiting">
              {task.waiting > 0
                ? t(task.counts_seasons ? "tasks.waiting_seasons" : "tasks.waiting", {
                    count: task.waiting,
                  })
                : t("tasks.nothing_waiting")}
            </span>
          )}
          {task.under_way ? (
            <span className="state-pill state-ok">
              <span className="state-dot" aria-hidden="true" />
              {t("tasks.under_way")}
            </span>
          ) : (
            <button className="button button-small" onClick={onStart}>
              <PlayIcon size={13} />
              {t("tasks.run")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

/*
 * When a task last ran, in one short phrase, with the time it took and how
 * it ended when it did not go well: a task that took four seconds and one
 * that took four hours are two different answers to "did it run last night".
 */
function lastRun(task: ScheduledTask, t: Wording): string {
  if (!task.last_run) {
    return t("tasks.never_run");
  }
  const when = whenItIs(task.last_run);
  const said =
    task.last_run_seconds === null
      ? t("tasks.last_run_unknown", { when })
      : t("tasks.last_run", { when, seconds: task.last_run_seconds });
  return task.last_run_state && task.last_run_state !== "succeeded"
    ? `${said} · ${t(`jobs.state.${task.last_run_state}`)}`
    : said;
}
