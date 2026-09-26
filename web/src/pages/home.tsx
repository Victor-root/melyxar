/*
 * The home page, as a set of rows rather than as one.
 *
 * The banner leads, then the sections below it in the order the viewer
 * chose in their settings, those they hid left out: the tiles leading to each
 * library, what was left halfway, what each started series is waiting on,
 * what arrived last, and the newest of each kind of library this server
 * really holds, a section each. The server says which, in which order. A row with nothing in it is
 * not drawn, so a server of films alone shows no empty row of anime and
 * nobody meets a heading standing over nothing.
 *
 * Every row is the same card, which is what makes a mark pressed in one of
 * them show in all the others at once.
 */

import { Fragment, useLayoutEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import type { Card as CardData, HomeSection, Library } from "../api";
import { Band } from "../components/band";
import { Card } from "../components/card";
import type { CardShape } from "../components/card";
import { Hero } from "../components/hero";
import { Row, RowHead } from "../components/row";
import { howFarIn, laidOut, useHomeScreen } from "../screens/home";
import { refusalKey } from "../i18n";
import { whatIsLeft, whichEpisode } from "../readable";
import { cardShapeOf, newestOfKind, whereAKindLeads } from "../libraries";
import { useSettings } from "../settings";
import { BinocularsIcon, CameraIcon, EyeIcon, KindIcon } from "../icons";

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
                <Link className="button" to="/admin/tasks">
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

  /* Each section as it is drawn, laid out below in the order the viewer
     chose. The row of a kind of library this server does not hold has
     nothing to draw, and is not drawn. */
  const sections: Partial<Record<HomeSection, React.ReactNode>> = {
    /* Where to go for somebody who already knows what they want. */
    band: <Band shelves={home.shelves} libraries={libraries} />,

    /* What was left halfway. Lying down, because what tells two of these
       apart is the still and the bar under it rather than the poster, and
       each one says how much of it is left and where it sits in its series:
       an episode title on its own is a title nobody placed. */
    carry_on: home.carry_on.length > 0 && (
      <section className="section">
        <RowHead mark={<EyeIcon size={24} />} title={t("home.carry_on")} />
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
    ),

    /* And what has not been started: the next episode of each series that
       is waiting on one, under the name of the series rather than under the
       episode's own, which nobody remembers. */
    up_next: home.up_next.length > 0 && (
      <section className="section">
        <RowHead mark={<BinocularsIcon size={24} />} title={t("home.up_next")} />
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
    ),

    recently_added: (
      <section className="section">
        <RowHead mark={<CameraIcon size={24} />} title={t("home.recently_added")} to={EVERYTHING_NEWEST}>
          {home.awaiting_identification > 0 && (
            <>
              <Link className="pill" to="/search?unidentified=true">
                {t("home.awaiting", { count: home.awaiting_identification })}
              </Link>
              {/* The one way to ask for it from a screen. Without this the
                  films sit there named after their file for ever, and the
                  only way out is a terminal. */}
              {jobs.length === 0 && (
                <button className="button button-small" onClick={lookUp.start} disabled={lookUp.starting}>
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
    ),

    /* The newest of each kind of library this server really holds, a
       section each. */
    ...Object.fromEntries(
      home.shelves.map((shelf) => [
        `newest:${shelf.kind}`,
        <Shelf
          title={newestOfKind(shelf.kind, libraries, t)}
          mark={<KindIcon kind={shelf.kind} size={24} />}
          cards={shelf.cards}
          shape={cardShapeOf(shelf.kind)}
          to={whereAKindLeads(shelf.kind, libraries)}
        />,
      ]),
    ),
  };

  return (
    <>
      <div className="home-backdrop drift" aria-hidden="true" />

      {/* Outside the page rather than inside it: the banner is the picture,
          and a picture held inside a column that stops short of both edges of
          a wide screen is a picture with a margin drawn round it. */}
      <Hero items={home.hero} />

      <main className="page page-home">
        {laidOut(home.sections).map((laid) =>
          typeof laid === "string" ? (
            <Fragment key={laid}>{sections[laid]}</Fragment>
          ) : (
            /* The two rows of what is already under way, side by side while
               both are short enough to go on one line. They are two rows
               either way, with a heading each; what they share is a line,
               and only while there is room for one. The stylesheet decides,
               on the width of the window it is being read in. */
            (sections[laid[0]] || sections[laid[1]]) && (
              <Together key={laid.join(":")}>
                {sections[laid[0]]}
                {sections[laid[1]]}
              </Together>
            )
          ),
        )}

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
 * The two rows of what is already under way, which share a line whenever
 * both are short enough for one to hold them.
 *
 * Whether they do is the stylesheet's answer rather than this file's: it
 * lays them side by side until the line runs out, on the width of the
 * window it is being read in. What is read back here is only that answer,
 * and only so a hairline can be drawn between them while they are on one
 * line. Down the side of a row that is on a line of its own, the same
 * hairline would be a rule drawn between nothing and nothing.
 */
function Together({ children }: { children: React.ReactNode }) {
  const box = useRef<HTMLDivElement>(null);
  const [sharing, setSharing] = useState(false);

  useLayoutEffect(() => {
    const it = box.current;
    if (!it) {
      return;
    }
    const look = () => {
      const rows = Array.from(it.children) as HTMLElement[];
      setSharing(rows.length === 2 && rows[0].offsetTop === rows[1].offsetTop);
    };
    look();

    /* Watched rather than worked out: what decides this is the width the
       rows are given and the width of what they hold, and both change
       without this page being drawn again. */
    const watch = new ResizeObserver(look);
    watch.observe(it);
    for (const row of Array.from(it.children)) {
      watch.observe(row);
    }
    return () => watch.disconnect();
  }, [children]);

  return (
    <div className={`rows-together${sharing ? " rows-sharing" : ""}`} ref={box}>
      {children}
    </div>
  );
}

/** One row of cards, which draws nothing at all when it holds nothing. */
function Shelf<T extends CardData>({
  title,
  mark,
  cards,
  shape,
  to,
}: {
  title: string;
  mark?: React.ReactNode;
  cards: T[];
  shape?: CardShape;
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
          <Card key={card.id} card={card} shape={shape} />
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
  const { bannerShown } = useSettings();
  return (
    <>
      <div className="home-backdrop drift" aria-hidden="true" />
      <main className="page page-home" aria-busy="true">
        {bannerShown && <div className="skeleton skeleton-hero" />}
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
