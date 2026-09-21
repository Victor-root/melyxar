/*
 * The home page, as a set of rows rather than as one.
 *
 * The order is the server's: what the page opens on, what was left halfway,
 * what each started series is waiting on, what arrived last, then one row per
 * kind of library this server really holds. A row with nothing in it is not
 * drawn, so a server of films alone shows no empty row of anime and nobody
 * meets a heading standing over nothing.
 *
 * Every row is the same card, which is what makes a mark pressed in one of
 * them show in all the others at once.
 *
 * Nothing here is arranged by hand yet. The order and what each row holds
 * come from the server, and the day somebody can turn a row off or move it
 * the pieces are already separate: that screen is a later worksite, and this
 * one is built so as not to be in its way.
 */

import { Link } from "react-router-dom";
import type { Card as CardData, Library } from "../api";
import { Card } from "../components/card";
import { Hero } from "../components/hero";
import { Row } from "../components/row";
import { JobLine } from "../components/job";
import { howFarIn, useHomeScreen } from "../screens/home";
import { refusalKey } from "../i18n";
import { useSettings } from "../settings";
import { ArrivedIcon, ClockIcon, SparkIcon } from "../icons";

export function HomePage({ libraries }: { libraries: Library[] }) {
  const { t } = useSettings();
  const { home, failed, again, jobs, scan, lookUp, refused } = useHomeScreen(libraries);

  if (failed) {
    return (
      <main className="page">
        <p className="notice">{t("error.unreachable")}</p>
        <button className="button" onClick={again}>
          {t("error.retry")}
        </button>
      </main>
    );
  }

  /* Skeletons rather than a spinner over the whole page: what is coming has a
     shape, and showing the shape is what stops the page jumping when it
     arrives. */
  if (!home) {
    return <HomeSkeleton />;
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
                <button
                  className="button button-accent"
                  onClick={scan.start}
                  disabled={scan.starting}
                >
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
    <main className="page page-home">
      <Hero items={home.hero} />

      {/* Work the server is doing, before anything suggested. It is the one
          thing here that is changing, and a scan of a whole library runs for
          hours: somebody who opens this page during one is opening it to see
          that. Buried between two rows of films it reads as one more row. */}
      {jobs.length > 0 && (
        <section className="section">
          <div className="jobs">
            {jobs.map((job) => (
              <JobLine key={job.id} job={job} />
            ))}
          </div>
        </section>
      )}

      {/* What was left halfway. Lying down, because what tells two of these
          apart is the still and the bar under it rather than the poster. */}
      <Shelf
        title={t("home.carry_on")}
        mark={<ClockIcon size={18} />}
        cards={home.carry_on}
        shape="lying"
        howFar={(card) => howFarIn(card.position_seconds, card.runtime_minutes)}
      />

      {/* And what has not been started: the next episode of each series that
          is waiting on one, under the name of the series rather than under
          the episode's own, which nobody remembers. */}
      {home.up_next.length > 0 && (
        <section className="section">
          <div className="section-head">
            <h2>
              <SparkIcon size={18} />
              {t("home.up_next")}
            </h2>
          </div>
          <Row>
            {home.up_next.map((card) => (
              <Card
                key={card.id}
                card={card}
                shape="lying"
                above={card.series_title}
                below={
                  card.season_number !== null && card.episode_number !== null
                    ? t("home.up_next.which", {
                        season: card.season_number,
                        episode: card.episode_number,
                      })
                    : undefined
                }
              />
            ))}
          </Row>
        </section>
      )}

      <section className="section">
        <div className="section-head">
          <h2>
            <ArrivedIcon size={18} />
            {t("home.recently_added")}
          </h2>
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
                  onClick={lookUp.start}
                  disabled={lookUp.starting}
                >
                  {t("home.identify")}
                </button>
              )}
            </>
          )}
        </div>
        <Row>
          {home.recently_added.map((card) => (
            <Card key={card.id} card={card} />
          ))}
        </Row>
      </section>

      {/* One row per kind of library this server really holds. */}
      {home.shelves.map((shelf) => (
        <Shelf key={shelf.kind} title={t(`home.newest.${shelf.kind}`)} cards={shelf.cards} />
      ))}

      {/* The server said no, which is an answer and belongs on the screen that
          asked rather than in a log nobody is reading. */}
      {refused && (
        <section className="section">
          <p className="notice">{t(refusalKey(refused))}</p>
        </section>
      )}
    </main>
  );
}

/** One row of cards, which draws nothing at all when it holds nothing. */
function Shelf<T extends CardData>({
  title,
  mark,
  cards,
  shape,
  howFar,
}: {
  title: string;
  mark?: React.ReactNode;
  cards: T[];
  shape?: "standing" | "lying";
  howFar?: (card: T) => number | undefined;
}) {
  if (cards.length === 0) {
    return null;
  }
  return (
    <section className="section">
      <div className="section-head">
        <h2>
          {mark}
          {title}
        </h2>
      </div>
      <Row>
        {cards.map((card) => (
          <Card key={card.id} card={card} shape={shape} watched={howFar?.(card)} />
        ))}
      </Row>
    </section>
  );
}

/**
 * The shape of the page, before the page.
 *
 * A spinner says something is happening; this says what is about to be there,
 * which is what stops everything jumping into place when it arrives.
 */
function HomeSkeleton() {
  return (
    <main className="page page-home" aria-busy="true">
      <div className="skeleton skeleton-hero" />
      {[0, 1].map((row) => (
        <section className="section" key={row}>
          <div className="skeleton skeleton-heading" />
          <div className="row-track">
            {[0, 1, 2, 3, 4, 5, 6].map((card) => (
              <div className="skeleton skeleton-card" key={card} />
            ))}
          </div>
        </section>
      ))}
    </main>
  );
}
