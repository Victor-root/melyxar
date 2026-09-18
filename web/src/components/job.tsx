/*
 * One piece of work the server is doing, drawn the same way everywhere.
 *
 * The activity page lists them all; the home page shows the scan it just
 * started. Two drawings of the same thing would drift apart.
 */

import type { Job } from "../api";
import { outOfAHundred } from "../readable";
import { useSettings } from "../settings";

export function JobLine({ job, onCancel }: { job: Job; onCancel?: () => void }) {
  const { t } = useSettings();

  return (
    <div className={`job job-${job.state}`}>
      <span className="job-kind">{t(`jobs.${job.kind}`)}</span>
      <span className="job-state">{t(`jobs.state.${job.state}`)}</span>
      {/* A scan is four passes end to end and the long ones are last, so a bar
          fills up, drops back to nothing and sets off again. Without a word
          saying which pass that is, it reads as a server that crashed and
          started over. */}
      {job.step && <span className="job-step">{t(`jobs.step.${job.step}`)}</span>}

      {job.ratio !== null ? (
        <>
          <span className="job-bar">
            <span className="job-bar-fill" style={{ width: `${outOfAHundred(job.ratio)}%` }} />
          </span>
          <span className="job-count">
            {job.done} / {job.total} · {outOfAHundred(job.ratio)} %
          </span>
        </>
      ) : (
        job.done > 0 && <span className="job-count">{job.done}</span>
      )}

      {/* Which file, right now. On a line of its own under the rest, whole: a
          pass name and a bar do not tell a server that is working from one
          stuck on a four hour film, and a name cut short tells which film only
          when two films do not begin alike. */}
      {job.doing && <span className="job-doing">{job.doing}</span>}

      {job.failure_reason && <span className="job-reason">{job.failure_reason}</span>}
      {onCancel && (
        <button className="button button-small" onClick={onCancel}>
          {t("jobs.cancel")}
        </button>
      )}
    </div>
  );
}
