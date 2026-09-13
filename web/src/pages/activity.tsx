/*
 * What the server is doing, and what it did.
 *
 * Refreshed while something is running and left alone when nothing is, so an
 * idle page does not poll a server all evening for no reason.
 */

import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import type { Jobs, Library } from "../api";
import { JobLine } from "../components/job";
import { CopyReport } from "../components/report";
import { useSettings } from "../settings";

/** How often a page showing running work asks again. */
const REFRESH_MS = 1500;

export function ActivityPage({ libraries }: { libraries: Library[] }) {
  const { t } = useSettings();
  const [jobs, setJobs] = useState<Jobs | null>(null);
  const [failed, setFailed] = useState(false);

  const load = useCallback(async (signal?: AbortSignal) => {
    try {
      setJobs(await api.jobs(signal));
      setFailed(false);
    } catch (error) {
      if (!(error instanceof DOMException)) {
        setFailed(true);
      }
    }
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    load(controller.signal);
    return () => controller.abort();
  }, [load]);

  const running = jobs?.running.length ?? 0;
  useEffect(() => {
    if (running === 0) {
      return;
    }
    const timer = window.setInterval(() => load(), REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [running, load]);

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
        {libraries.map((library) => (
          <span key={library.id} className="control-group">
            <button
              className="button"
              onClick={() => api.scan(library.id).then(() => load())}
            >
              {t("home.scan")} · {library.name}
            </button>
            <button
              className="button"
              onClick={() => api.identify(library.id).then(() => load())}
            >
              {t("home.identify")}
            </button>
          </span>
        ))}
      </div>

      {failed && <p className="notice">{t("error.unreachable")}</p>}

      <section className="section">
        <h2>{t("jobs.running")}</h2>
        {running === 0 ? (
          <p className="notice notice-faint">{t("jobs.none")}</p>
        ) : (
          jobs?.running.map((job) => (
            <JobLine key={job.id} job={job} onCancel={() => api.cancelJob(job.id).then(() => load())} />
          ))
        )}
      </section>

      {jobs && jobs.recent.length > 0 && (
        <section className="section">
          <div className="section-head">
            <h2>{t("jobs.recent")}</h2>
            {/* Kept next to the list it empties, and only shown when there is
                something to empty. */}
            <button
              className="button button-small"
              onClick={() => api.forgetFinishedJobs().then(() => load())}
            >
              {t("jobs.forget")}
            </button>
          </div>
          {jobs.recent.map((job) => (
            <JobLine key={job.id} job={job} />
          ))}
        </section>
      )}
    </main>
  );
}

