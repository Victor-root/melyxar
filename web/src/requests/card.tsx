/*
 * One title as the requests show it: its poster, its name and year, whether
 * it is a film or a series, a few lines of what it is about, and beside
 * them whatever the screen has to say of it or do with it.
 */

import { useState } from "react";
import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import { FilmIcon } from "../icons";
import { useSettings } from "../settings";
import type { Catalogue } from "./api";

/** Where the whole page of a title is. */
export function titleAddress(catalogue: Catalogue, tmdbId: string): string {
  return `/requests/title/${catalogue}/${encodeURIComponent(tmdbId)}`;
}

export function TitleCard({
  catalogue,
  title,
  year,
  poster,
  overview,
  to,
  children,
}: {
  catalogue: Catalogue;
  title: string;
  year: number | null;
  poster: string | null;
  overview?: string | null;
  /** Where pressing the poster or the name leads: the page of the title. */
  to?: string;
  children?: ReactNode;
}) {
  const { t } = useSettings();
  /* A poster the provider does not hand over leaves the mark of a title
     without one, never a broken picture. */
  const [failed, setFailed] = useState(false);
  const picture = (
    <span className="request-poster">
      {poster && !failed ? (
        <img src={poster} alt="" loading="lazy" decoding="async" onError={() => setFailed(true)} />
      ) : (
        <FilmIcon size={28} />
      )}
    </span>
  );
  return (
    <article className="request-card">
      {to ? (
        <Link to={to} tabIndex={-1} aria-hidden="true">
          {picture}
        </Link>
      ) : (
        picture
      )}
      <div className="request-words">
        <h3 className="request-title">
          {to ? (
            <Link to={to} className="request-title-link">
              {title}
            </Link>
          ) : (
            title
          )}
          {year !== null && <span className="request-year">{year}</span>}
        </h3>
        <span className="request-kind">{t(`requests.catalogue.${catalogue}`)}</span>
        {overview && <p className="request-overview">{overview}</p>}
        {children}
      </div>
    </article>
  );
}
