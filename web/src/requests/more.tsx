/*
 * Many more titles of one shelf of suggestions: the most popular of a
 * catalogue, in a genre when one is named, a page at a time as far as the
 * provider goes, the next one read as the end of the list comes near.
 * Reached from "see more" under a shelf.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { useParams, useSearchParams } from "react-router-dom";
import { useReachEnd } from "../components/reach-end";
import { useSettings } from "../settings";
import { Answer } from "./answer";
import { requestsApi } from "./api";
import type { Catalogue, Found } from "./api";
import { AskDialog } from "./ask";

export function MoreTitles() {
  const { t } = useSettings();
  const { catalogue } = useParams();
  const [parameters] = useSearchParams();
  if (catalogue !== "films" && catalogue !== "series") {
    return <p className="notice">{t("error.not_found")}</p>;
  }
  const genre = parameters.get("genre");
  return (
    <MoreList
      key={`${catalogue}:${genre ?? ""}`}
      catalogue={catalogue}
      genre={genre}
      name={parameters.get("name")}
    />
  );
}

function MoreList({ catalogue, genre, name }: { catalogue: Catalogue; genre: string | null; name: string | null }) {
  const { t, language } = useSettings();
  const [titles, setTitles] = useState<Found[]>([]);
  const [page, setPage] = useState(1);
  const [ended, setEnded] = useState(false);
  const [waiting, setWaiting] = useState(true);
  const [asking, setAsking] = useState<Found | null>(null);
  const end = useRef<HTMLDivElement>(null);
  const readNext = useCallback(() => setPage((was) => was + 1), []);

  // The next page is read as the end comes near, one at a time, and the end
  // is watched again once a page is in, in case the list is still short.
  useReachEnd(end, waiting || ended ? null : readNext, 800);

  useEffect(() => {
    const stop = new AbortController();
    setWaiting(true);
    requestsApi
      .popular(catalogue, genre, page, language, stop.signal)
      .then((more) => {
        setTitles((held) => {
          const known = new Set(held.map((one) => one.tmdb_id));
          return [...held, ...more.filter((one) => !known.has(one.tmdb_id))];
        });
        setEnded(more.length === 0);
        setWaiting(false);
      })
      .catch(() => {
        if (!stop.signal.aborted) {
          setEnded(true);
          setWaiting(false);
        }
      });
    return () => stop.abort();
  }, [catalogue, genre, page, language]);

  return (
    <>
      <div className="section-head request-more-head">
        <h2>{name ? t("requests.suggested_genre", { genre: name }) : t("requests.suggested_popular")}</h2>
        <span className="request-kind">{t(`requests.catalogue.${catalogue}`)}</span>
      </div>
      <div className="request-grid">
        {titles.map((one) => (
          <Answer key={`${one.catalogue}:${one.tmdb_id}`} found={one} onAsk={() => setAsking(one)} />
        ))}
      </div>
      <div ref={end} />
      {asking && <AskDialog found={asking} onClose={() => setAsking(null)} />}
    </>
  );
}
