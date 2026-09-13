/*
 * The home page: what arrived last, and a way to fill the library when there
 * is nothing yet.
 */

import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api } from "../api";
import type { Home as HomeData, Library } from "../api";
import { Card } from "../components/card";
import { Grid } from "../components/grid";
import { JobLine } from "../components/job";
import { useRunning, useStartScan } from "../running";
import { useSettings } from "../settings";

export function HomePage({ libraries }: { libraries: Library[] }) {
  const { t } = useSettings();
  const [home, setHome] = useState<HomeData | null>(null);
  const [failed, setFailed] = useState(false);
  const [working, setWorking] = useState(false);

  const load = useCallback((signal?: AbortSignal) => {
    setFailed(false);
    api
      .home(undefined, signal)
      .then(setHome)
      .catch((error) => {
        if (!(error instanceof DOMException)) {
          setFailed(true);
        }
      });
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    load(controller.signal);
    return () => controller.abort();
  }, [load]);

  /* Work takes minutes on a real library, so the page reads again what it
     produced when it ends. Without this, pressing a button looks exactly like
     pressing a button that does nothing. */
  const { jobs, watch, finished } = useRunning();
  const { start: startScan, starting } = useStartScan(libraries);
  useEffect(() => {
    if (finished > 0) {
      load();
    }
  }, [finished, load]);

  /* Asked for on its own, for the films a previous look up did not name:
     a provider that was down, a title nobody recognised, a key added since. */
  const startIdentification = async () => {
    setWorking(true);
    try {
      await Promise.all(libraries.map((library) => api.identify(library.id)));
      watch();
    } finally {
      setWorking(false);
    }
  };

  if (failed) {
    return (
      <main className="page">
        <p className="notice">{t("error.unreachable")}</p>
        <button className="button" onClick={() => load()}>
          {t("error.retry")}
        </button>
      </main>
    );
  }

  if (!home) {
    return <main className="page" aria-busy="true" />;
  }

  if (home.works === 0) {
    return (
      <main className="page">
        <section className="empty">
          <h1>{t("home.empty.title")}</h1>
          {/* While the scan runs, what it is doing replaces the invitation to
              start one: reading "run a scan" during a scan is what sends
              somebody to press the button a second time. */}
          {jobs.length > 0 ? (
            <>
              <p>{t("home.scanning")}</p>
              <div className="jobs">
                {jobs.map((job) => (
                  <JobLine key={job.id} job={job} />
                ))}
              </div>
            </>
          ) : (
            <>
              <p>{t("home.empty.body")}</p>
              {libraries.length > 0 && (
                <button className="button button-accent" onClick={startScan} disabled={starting}>
                  {t("home.scan")}
                </button>
              )}
            </>
          )}
        </section>
      </main>
    );
  }

  return (
    <main className="page">
      {/* Work the server is doing, shown where somebody lands rather than on a
          page they have to think of opening. */}
      {jobs.length > 0 && (
        <section className="section">
          <div className="jobs">
            {jobs.map((job) => (
              <JobLine key={job.id} job={job} />
            ))}
          </div>
        </section>
      )}

      <section className="section">
        <div className="section-head">
          <h1>{t("home.recently_added")}</h1>
          {home.awaiting_identification > 0 && (
            <>
              <Link className="pill" to="/search?unidentified=true">
                {t("home.awaiting", { count: home.awaiting_identification })}
              </Link>
              {/* The one way to ask for it from a screen. Without this the
                  films sit there named after their file for ever, and the
                  only way out is a terminal. */}
              {jobs.length === 0 && (
                <button
                  className="button button-small"
                  onClick={startIdentification}
                  disabled={working}
                >
                  {t("home.identify")}
                </button>
              )}
            </>
          )}
        </div>
        <Grid>
          {home.recently_added.map((card) => (
            <Card key={card.id} card={card} />
          ))}
        </Grid>
      </section>

      <section className="section">
        <div className="section-head">
          <h2>{t("nav.libraries")}</h2>
        </div>
        <div className="library-row">
          {libraries.map((library) => (
            <Link key={library.id} className="library-tile" to={`/library/${library.id}`}>
              <span className="library-name">{library.name}</span>
              <span className="library-count">
                {t("library.count", { count: library.works })}
              </span>
              {library.roots.map((root) => (
                <span
                  key={root.label}
                  className={`root-state root-${root.access}`}
                  title={t(`root.${root.explanation_code}`)}
                >
                  {root.label}
                </span>
              ))}
            </Link>
          ))}
        </div>
      </section>
    </main>
  );
}
