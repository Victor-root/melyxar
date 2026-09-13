/*
 * One piece of work the server is doing, drawn the same way everywhere.
 *
 * The activity page lists them all; the home page shows the scan it just
 * started. Two drawings of the same thing would drift apart.
 */

import type { Job } from "../api";
import { useSettings } from "../settings";

export function JobLine({ job, onCancel }: { job: Job; onCancel?: () => void }) {
  const { t } = useSettings();

  return (
    <div className={`job job-${job.state}`}>
      <span className="job-kind">{t(`jobs.${job.kind}`)}</span>
      <span className="job-state">{t(`jobs.state.${job.state}`)}</span>

      {job.ratio !== null ? (
        <span className="job-bar">
          <span className="job-bar-fill" style={{ width: `${Math.round(job.ratio * 100)}%` }} />
          <span className="job-bar-text">
            {job.done} / {job.total}
          </span>
        </span>
      ) : (
        job.done > 0 && <span className="job-bar-text">{job.done}</span>
      )}

      {job.failure_reason && <span className="job-reason">{job.failure_reason}</span>}
      {onCancel && (
        <button className="button button-small" onClick={onCancel}>
          {t("jobs.cancel")}
        </button>
      )}
    </div>
  );
}
