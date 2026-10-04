/*
 * One title as the requests show it: its poster, its name and year, whether
 * it is a film or a series, a few lines of what it is about, and beside
 * them whatever the screen has to say of it or do with it.
 */

import { useState } from "react";
import type { ReactNode } from "react";
import { FilmIcon } from "../icons";
import { useSettings } from "../settings";
import type { Catalogue } from "./api";

export function TitleCard({
  catalogue,
  title,
  year,
  poster,
  overview,
  children,
}: {
  catalogue: Catalogue;
  title: string;
  year: number | null;
  poster: string | null;
  overview?: string | null;
  children?: ReactNode;
}) {
  const { t } = useSettings();
  /* A poster the provider does not hand over leaves the mark of a title
     without one, never a broken picture. */
  const [failed, setFailed] = useState(false);
  return (
    <article className="request-card">
      <span className="request-poster">
        {poster && !failed ? (
          <img src={poster} alt="" loading="lazy" decoding="async" onError={() => setFailed(true)} />
        ) : (
          <FilmIcon size={28} />
        )}
      </span>
      <div className="request-words">
        <h3 className="request-title">
          {title}
          {year !== null && <span className="request-year">{year}</span>}
        </h3>
        <span className="request-kind">{t(`requests.catalogue.${catalogue}`)}</span>
        {overview && <p className="request-overview">{overview}</p>}
        {children}
      </div>
    </article>
  );
}
