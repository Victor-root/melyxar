/*
 * The page of one person.
 *
 * Their face, when they were born and where, their life in a few lines, and
 * then what of theirs this server holds, films and series in a row each, the
 * same rows as everywhere else. The top is one screen, the way the page of a
 * work is: the life gives way when the screen is short, and says so.
 */

import { useParams } from "react-router-dom";
import { api } from "../api";
import type { Person } from "../api";
import { useAsked } from "../asking";
import { Card } from "../components/card";
import { useShownPicture } from "../components/picture";
import { Row, RowHead } from "../components/row";
import { useFittedText } from "../fitting";
import { FilmIcon, SeriesIcon } from "../icons";
import { readableDay, todayOf, yearsBetween } from "../readable";
import { useSettings } from "../settings";

export function PersonPage() {
  const { id } = useParams();
  const { t } = useSettings();
  const asked = useAsked(
    (signal) => (id ? api.person(id, signal) : Promise.resolve(null)),
    [id],
    id ? `person:${id}` : undefined,
  );
  const person = asked.waiting ? null : asked.answer;

  if (asked.failure) {
    return (
      <main className="page">
        <p className="notice">
          {t(asked.failure.code === "not_found" ? "error.not_found" : "error.unreachable")}
        </p>
      </main>
    );
  }
  /* The first opening waits on the provider, once: the shape of the page
     stands in the meantime rather than an empty screen. */
  if (!person) {
    return <main className="page" aria-busy="true" />;
  }

  const films = person.works.filter((card) => card.kind === "movie");
  const series = person.works.filter((card) => card.kind !== "movie");

  return (
    <>
      <div className="home-backdrop drift" aria-hidden="true" />
      <main className="work person">
        <AboutThem person={person} />

        {films.length > 0 && (
          <section className="section">
            <RowHead mark={<FilmIcon size={24} />} title={t("person.films")} />
            <Row>
              {films.map((card) => (
                <Card key={card.id} card={card} />
              ))}
            </Row>
          </section>
        )}

        {series.length > 0 && (
          <section className="section">
            <RowHead mark={<SeriesIcon size={24} />} title={t("person.series")} />
            <Row>
              {series.map((card) => (
                <Card key={card.id} card={card} />
              ))}
            </Row>
          </section>
        )}
      </main>
    </>
  );
}

/** Who they are: the top of the page, which fits the screen. */
function AboutThem({ person }: { person: Person }) {
  const { t, language } = useSettings();
  const { picture: photo, itDidNotLoad } = useShownPicture(person.photo);
  const fitted = useFittedText([person.id, person.biography]);

  /* How old they are, or were: counted to the day they died when they did,
     to today on the reader's own calendar otherwise. */
  const until = person.died_on ?? todayOf(new Date());
  const age = person.born_on ? yearsBetween(person.born_on, until) : null;
  const born = person.born_on ? readableDay(person.born_on, language) : null;
  const died = person.died_on ? readableDay(person.died_on, language) : null;

  return (
    <section className="work-top">
      <div className="work-poster">
        {photo ? (
          <img
            src={photo.src}
            srcSet={photo.srcSet}
            sizes="(max-width: 800px) 40vw, 300px"
            alt=""
            onError={itDidNotLoad}
          />
        ) : (
          <div className="work-poster-empty" aria-hidden="true">
            {person.name.slice(0, 1)}
          </div>
        )}
      </div>

      <div
        className={`work-body${fitted.fit.wide ? " work-body-wide" : ""}`}
        ref={fitted.column}
      >
        <div className="work-room" ref={fitted.ruler} aria-hidden="true" />
        <h1 className="work-title">{person.name}</h1>

        {(born || died || person.birthplace) && (
          <dl className="work-credits">
            {born && (
              <div className="work-credit">
                <dt>{t("person.born")}</dt>
                <dd>
                  {died || age === null ? born : t("person.born_age", { day: born, age })}
                </dd>
              </div>
            )}
            {died && (
              <div className="work-credit">
                <dt>{t("person.died")}</dt>
                <dd>{age === null ? died : t("person.died_age", { day: died, age })}</dd>
              </div>
            )}
            {person.birthplace && (
              <div className="work-credit">
                <dt>{t("person.birthplace")}</dt>
                <dd>{person.birthplace}</dd>
              </div>
            )}
          </dl>
        )}

        {person.biography ? (
          <div className="work-synopsis">
            <p
              ref={fitted.text}
              className="work-overview"
              style={fitted.style}
              data-cut={fitted.cut ? "" : undefined}
            >
              {person.biography}
            </p>
            {fitted.fit.lines !== null && (
              <button className="link-button" onClick={fitted.toggle}>
                {t(fitted.open ? "work.less" : "work.more")}
              </button>
            )}
          </div>
        ) : (
          <p className="work-overview work-overview-empty">{t("person.no_biography")}</p>
        )}
      </div>
    </section>
  );
}
