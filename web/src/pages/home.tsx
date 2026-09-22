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
import { Band } from "../components/band";
import { Card } from "../components/card";
import { Hero } from "../components/hero";
import { Row } from "../components/row";
import { howFarIn, useHomeScreen } from "../screens/home";
import { refusalKey } from "../i18n";
import { howLong, whichEpisode } from "../readable";
import { whereAKindLeads } from "../libraries";
import { useSettings } from "../settings";
import { ArrivedIcon, ChevronRightIcon, ClockIcon, KindIcon, SparkIcon } from "../icons";

/** Where the row of everything newest leads, which is the same grid read in
 *  the same order. */
const EVERYTHING_NEWEST = "/search?order=added_at&descending=true";

export function HomePage({ libraries }: { libraries: Library[] }) {
  const { t } = useSettings();
  const { home, failed, again, jobs, scan, lookUp, refused } = useHomeScreen(libraries);

  if (failed) {
    return (
      <>
        <div className="home-backdrop drift" aria-hidden="true" />
        <main className="page">
          <p className="notice">{t("error.unreachable")}</p>
          <button className="button" onClick={again}>
            {t("error.retry")}
          </button>
        </main>
      </>
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
      <>
        <div className="home-backdrop drift" aria-hidden="true" />
        <main className="page">
          <section className="empty">
            <h1>{t("home.empty.title")}</h1>
            {/* While the scan runs, what it is doing replaces the invitation
                to start one: reading "run a scan" during a scan is what
                sends somebody to press the button a second time. How far
                along it is belongs to the screen that exists for it, and
                the way there is said rather than left to be found. */}
            {jobs.length > 0 ? (
              <>
                <p>{t("home.scanning")}</p>
                <Link className="button" to="/activity">
                  {t("nav.jobs")}
                </Link>
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
      </>
    );
  }

  return (
    <>
      <div className="home-backdrop drift" aria-hidden="true" />

      {/* Outside the page rather than inside it: the banner is the picture,
          and a picture held inside a column that stops short of both edges of
          a wide screen is a picture with a margin drawn round it. */}
      <Hero items={home.hero} />

      <main className="page page-home">
        {/* Where to go for somebody who already knows what they want, before
            any row of suggestions. */}
        <Band shelves={home.shelves} libraries={libraries} />

        {/* The two rows of what is already under way, side by side while
            both are short enough to go on one line. They are two rows
            either way, with a heading each; what they share is a line, and
            only while there is room for one. The stylesheet decides, on the
            width of the window it is being read in. */}
        {(home.carry_on.length > 0 || home.up_next.length > 0) && (
          <div className="rows-together">
            {/* What was left halfway. Lying down, because what tells two of
                these apart is the still and the bar under it rather than
                the poster, and each one says how much of it is left and
                where it sits in its series: an episode title on its own is
                a title nobody placed. */}
            {home.carry_on.length > 0 && (
              <section className="section">
                <RowHead mark={<ClockIcon size={18} />} title={t("home.carry_on")} />
                <Row>
                  {home.carry_on.map((card) => (
                    <Card
                      key={card.id}
                      card={card}
                      shape="lying"
                      watched={howFarIn(card.position_seconds, card.runtime_minutes)}
                      lead={card.series_title ?? undefined}
                      note={whichEpisode(card, t)}
                      trailing={whatIsLeft(card.position_seconds, card.runtime_minutes, t)}
                    />
                  ))}
                </Row>
              </section>
            )}

            {/* And what has not been started: the next episode of each
                series that is waiting on one, under the name of the series
                rather than under the episode's own, which nobody
                remembers. */}
            {home.up_next.length > 0 && (
              <section className="section">
                <RowHead mark={<SparkIcon size={18} />} title={t("home.up_next")} />
                <Row>
                  {home.up_next.map((card) => (
                    <Card
                      key={card.id}
                      card={card}
                      shape="lying"
                      lead={card.series_title}
                      note={whichEpisode(card, t)}
                    />
                  ))}
                </Row>
              </section>
            )}
          </div>
        )}

        <section className="section">
          <RowHead
            mark={<ArrivedIcon size={18} />}
            title={t("home.recently_added")}
            to={EVERYTHING_NEWEST}
          >
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
          </RowHead>
          <Row>
            {home.recently_added.map((card) => (
              <Card key={card.id} card={card} />
            ))}
          </Row>
        </section>

        {/* One row per kind of library this server really holds. */}
        {home.shelves.map((shelf) => (
          <Shelf
            key={shelf.kind}
            title={t(`home.newest.${shelf.kind}`)}
            mark={<KindIcon kind={shelf.kind} size={18} />}
            cards={shelf.cards}
            to={whereAKindLeads(shelf.kind, libraries)}
          />
        ))}

        {/* The server said no, which is an answer and belongs on the screen
            that asked rather than in a log nobody is reading. */}
        {refused && (
          <section className="section">
            <p className="notice">{t(refusalKey(refused))}</p>
          </section>
        )}
      </main>
    </>
  );
}

/**
 * The heading of one row.
 *
 * A row that has a whole grid behind it says so twice: the heading itself
 * leads there, carrying the chevron that says it is a way through, and the
 * words at the right say it in words for whoever does not read chevrons. A
 * row that is only ever a row, like what somebody left halfway, has neither,
 * because a chevron leading nowhere is worse than no chevron.
 */
function RowHead({
  mark,
  title,
  to,
  children,
}: {
  mark?: React.ReactNode;
  title: string;
  to?: string;
  children?: React.ReactNode;
}) {
  const { t } = useSettings();

  return (
    <div className="section-head">
      <h2>
        {mark}
        {to ? (
          <Link className="section-through" to={to}>
            {title}
            <ChevronRightIcon size={17} />
          </Link>
        ) : (
          title
        )}
      </h2>
      {children}
      {to && (
        <Link className="section-all" to={to}>
          {t("home.see_all")}
        </Link>
      )}
    </div>
  );
}

/** One row of cards, which draws nothing at all when it holds nothing. */
function Shelf<T extends CardData>({
  title,
  mark,
  cards,
  to,
}: {
  title: string;
  mark?: React.ReactNode;
  cards: T[];
  to?: string;
}) {
  if (cards.length === 0) {
    return null;
  }
  return (
    <section className="section">
      <RowHead mark={mark} title={title} to={to} />
      <Row>
        {cards.map((card) => (
          <Card key={card.id} card={card} />
        ))}
      </Row>
    </section>
  );
}

/** How much of a film is left, which is what a row of half watched ones is
 *  read for. */
function whatIsLeft(
  seconds: number,
  runtimeMinutes: number | null,
  t: ReturnType<typeof useSettings>["t"],
): string | undefined {
  if (!runtimeMinutes || runtimeMinutes <= 0) {
    return undefined;
  }
  const left = Math.max(Math.round(runtimeMinutes - seconds / 60), 0);
  return left === 0 ? undefined : t("home.hero.left", { time: howLong(left, t) });
}

/**
 * The shape of the page, before the page.
 *
 * A spinner says something is happening; this says what is about to be there,
 * which is what stops everything jumping into place when it arrives.
 */
function HomeSkeleton() {
  return (
    <>
      <div className="home-backdrop drift" aria-hidden="true" />
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
    </>
  );
}
