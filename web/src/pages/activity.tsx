/*
 * What the server is doing, and what it did.
 *
 * What is running is taken from the one place that watches it, rather than
 * asked for again here. This page used to ask on its own and start looking
 * again only once it had seen something running, so work started anywhere else
 * never appeared: the bar at the top said a scan was under way and this page,
 * two centimetres below it, said nothing was. Two answers to one question is
 * one answer too many, and the wrong one is always the one somebody reads.
 *
 * What finished is this page's own business, and is asked for again whenever
 * the work being watched comes to an end.
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "../api";
import type { Job, Library } from "../api";
import { JobLine } from "../components/job";
import { CopyReport } from "../components/report";
import { refusalKey } from "../i18n";
import { useRunning } from "../running";
import { useSettings } from "../settings";

export function ActivityPage({ libraries }: { libraries: Library[] }) {
  const { t } = useSettings();
  const { jobs: running, watch, finished } = useRunning();
  const [recent, setRecent] = useState<Job[]>([]);
  const [failed, setFailed] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);

  /* One library at a time, which is what this page is for: the bar at the top
     already offers the whole lot at once, and somebody who came here came to
     act on one thing. Whatever is started is watched at once rather than left
     to the slow beat, because a button that shows nothing for twenty seconds
     is indistinguishable from a button that did nothing. */
  const startOn = useCallback(
    async (asked: Promise<unknown>) => {
      setRefused(null);
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

  /* On the way in, and again each time the work being watched comes to an
     end: that is exactly when something has moved from running to finished. */
  useEffect(() => {
    const controller = new AbortController();
    loadRecent(controller.signal);
    return () => controller.abort();
  }, [loadRecent, finished]);


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
            <button className="button" onClick={() => startOn(api.scan(library.id))}>
              {t("home.scan")} · {library.name}
            </button>
            <button className="button" onClick={() => startOn(api.identify(library.id))}>
              {t("home.identify")}
            </button>
          </span>
        ))}
      </div>

      {/* A button that fails in silence is the same thing as a button that
          does nothing, and sends somebody to a terminal. */}
      {refused && <p className="notice">{t(refusalKey(refused))}</p>}
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
