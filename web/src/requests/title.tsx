/*
 * The whole page of one title that may not be on the server, read the way
 * the page of a title that is: its backdrop and poster, what it is, what it
 * is about, who made it and who is in it, and for a series its seasons. What
 * is said comes from the provider; what is offered beside it is asking for
 * the title, or opening it once it is here.
 */

import { useState } from "react";
import { useParams } from "react-router-dom";
import { useAsked } from "../asking";
import { Row, RowHead } from "../components/row";
import { TrailerDialog } from "../components/trailer";
import { PeopleIcon, FilmIcon, TrailerIcon } from "../icons";
import { elsewhere, groupCrew } from "../screens/work";
import { howLong, howMany, outOfTen, readableDay, numberOfOne } from "../readable";
import { useSettings } from "../settings";
import { embedOf } from "../trailers";
import { requestsApi } from "./api";
import type { Catalogue, TitlePerson } from "./api";
import { AskDialog } from "./ask";
import { StandingBar } from "./standing-bar";
import { useRequests } from "./store";

/** How many of the cast are shown. */
const FACES_SHOWN = 18;

function Face({ person }: { person: TitlePerson }) {
  const [failed, setFailed] = useState(false);
  return (
    <article className="card card-standing card-person" data-card>
      <div className="card-picture">
        {person.photo && !failed ? (
          <img
            src={person.photo}
            alt=""
            loading="lazy"
            decoding="async"
            draggable={false}
            onError={() => setFailed(true)}
          />
        ) : (
          <span className="card-initial" aria-hidden="true">
            {person.name.slice(0, 1)}
          </span>
        )}
      </div>
      <span className="card-line">
        <span className="card-title">{person.name}</span>
      </span>
      <span className="card-year">{person.character ?? ""}</span>
    </article>
  );
}

export function RequestTitlePage() {
  const { catalogue, id } = useParams();
  const { t, language } = useSettings();
  const { access } = useRequests();
  const title = useAsked(
    (signal) => requestsApi.title(catalogue as Catalogue, id ?? "", language, signal),
    [catalogue, id, language],
    `request-title:${language}:${catalogue}:${id}`,
  );
  const [asking, setAsking] = useState(false);
  const [trailer, setTrailer] = useState<{ embed: string } | null>(null);
  /* A picture the provider does not hand over leaves the page as it is for a
     title without one, never a broken picture. */
  const [posterFailed, setPosterFailed] = useState(false);
  const [backdropFailed, setBackdropFailed] = useState(false);

  if (access === null) {
    return null;
  }
  if (!access.may_ask || (catalogue !== "films" && catalogue !== "series")) {
    return (
      <main className="page">
        <p className="notice">{t("error.not_found")}</p>
      </main>
    );
  }
  const page = title.answer;
  if (!page) {
    return (
      <main className="page" aria-busy={title.failure ? undefined : "true"}>
        {title.failure && <p className="notice">{t("error.unreachable")}</p>}
      </main>
    );
  }

  const { found } = page;
  const kind = catalogue === "films" ? "movie" : "series";
  const day = page.release_date ? readableDay(page.release_date, language) : null;
  const facts = [
    day ?? (found.year !== null ? String(found.year) : null),
    page.seasons.length > 0 ? howMany(page.seasons.length, "work.season_count", t) : null,
    page.runtime_minutes ? howLong(page.runtime_minutes, t) : null,
  ].filter((fact): fact is string => Boolean(fact));
  const links = [
    { provider: "tmdb", address: elsewhere("tmdb", found.tmdb_id, kind) },
    ...(page.imdb_id ? [{ provider: "imdb", address: elsewhere("imdb", page.imdb_id, kind) }] : []),
  ];
  const embed = page.trailers.map(embedOf).find((address) => address !== null) ?? null;
  const away = embed ? null : (page.trailers[0] ?? null);

  return (
    <main className="work">
      {page.backdrop && !backdropFailed && (
        <div className="work-backdrop" aria-hidden="true">
          <img src={page.backdrop} alt="" fetchPriority="high" onError={() => setBackdropFailed(true)} />
        </div>
      )}

      <section className="work-top">
        <div className="work-poster">
          {page.big_poster && !posterFailed ? (
            <img src={page.big_poster} alt="" onError={() => setPosterFailed(true)} />
          ) : (
            <div className="work-poster-empty" aria-hidden="true">
              <FilmIcon size={64} />
            </div>
          )}
        </div>

        <div className="work-body">
          <h1 className="work-title">{found.title}</h1>
          {page.tagline && <p className="work-tagline">{page.tagline}</p>}

          <p className="work-facts">
            {page.rating !== null && page.rating > 0 && (
              <span className="work-rating" title={t("rating.tmdb")}>
                <span className="work-rating-mark work-rating-tmdb" aria-hidden="true">
                  TMDB
                </span>
                {outOfTen(page.rating)}
              </span>
            )}
            {facts.length > 0 && <span>{facts.join(" · ")}</span>}
            {page.age_rating && <span className="work-badge">{page.age_rating}</span>}
            {page.genres.length > 0 && <span>{page.genres.join(", ")}</span>}
          </p>

          <div className="work-actions">
            <StandingBar found={found} onAsk={() => setAsking(true)} />
            {embed && (
              <button type="button" className="button button-large" onClick={() => setTrailer({ embed })}>
                <TrailerIcon size={22} />
                {t("work.trailer")}
              </button>
            )}
            {away && (
              <a className="button button-large" href={away} target="_blank" rel="noreferrer noopener">
                <TrailerIcon size={22} />
                {t("work.trailer")}
              </a>
            )}
          </div>

          {found.overview ? (
            <div className="work-synopsis">
              <p className="work-overview">{found.overview}</p>
            </div>
          ) : (
            <p className="work-overview work-overview-empty">{t("work.no_overview")}</p>
          )}

          <dl className="work-credits">
            {groupCrew(page.crew).map(([role, names]) => (
              <div key={role} className="work-credit">
                <dt>{t(`credit.${role}`)}</dt>
                <dd>{names.join(", ")}</dd>
              </div>
            ))}
            {page.studios.length > 0 && (
              <div className="work-credit">
                <dt>{t("media.studios")}</dt>
                <dd>{page.studios.join(", ")}</dd>
              </div>
            )}
            {page.collection && (
              <div className="work-credit">
                <dt>{t("work.saga")}</dt>
                <dd>{page.collection}</dd>
              </div>
            )}
            <div className="work-credit">
              <dt>{t("media.links")}</dt>
              <dd className="work-links">
                {links.map((link) => (
                  <a
                    key={link.provider}
                    className="work-link"
                    href={link.address}
                    target="_blank"
                    rel="noreferrer noopener"
                  >
                    {t(`provider.${link.provider}`)}
                  </a>
                ))}
              </dd>
            </div>
          </dl>
        </div>
      </section>

      {page.seasons.length > 0 && (
        <section className="section">
          <RowHead mark={<FilmIcon size={24} />} title={t("work.seasons")} />
          <ul className="request-season-list">
            {page.seasons.map((season) => (
              <li key={season.number} className="request-season-item">
                <span className="request-season-name">{numberOfOne("season", season.number, t)}</span>
                <span className="request-others">{t("requests.episodes", { count: season.episodes })}</span>
                {season.held && <span className="state-pill state-ok">{t("requests.season_here")}</span>}
              </li>
            ))}
          </ul>
        </section>
      )}

      {page.cast.length > 0 && (
        <section className="section">
          <RowHead mark={<PeopleIcon size={24} />} title={t("work.cast")} />
          <Row>
            {page.cast.slice(0, FACES_SHOWN).map((person, index) => (
              <Face key={`${person.name}-${index}`} person={person} />
            ))}
          </Row>
        </section>
      )}

      {asking && <AskDialog found={found} onClose={() => setAsking(false)} />}
      {trailer && <TrailerDialog trailer={trailer} title={found.title} onClose={() => setTrailer(null)} />}
    </main>
  );
}
