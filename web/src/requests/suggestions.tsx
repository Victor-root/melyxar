/*
 * What the page of requests shows before anything is typed: two lines of
 * titles for each of the genres this account watches most, or for the most
 * popular ones while it has not watched enough to tell, and under each the
 * way to many more. Each is asked for the same way as an answer of a search.
 */

import { useState } from "react";
import { useAsked } from "../asking";
import { RowHead } from "../components/row";
import { useSettings } from "../settings";
import { Answer } from "./answer";
import { requestsApi } from "./api";
import type { Catalogue, Found } from "./api";
import { AskDialog } from "./ask";

/** Where seeing more of a shelf leads. */
export function moreAddress(catalogue: Catalogue, genreId: string | null, genre: string | null): string {
  const parameters = new URLSearchParams();
  if (genreId && genre) {
    parameters.set("genre", genreId);
    parameters.set("name", genre);
  }
  const text = parameters.toString();
  return `/requests/more/${catalogue}${text ? `?${text}` : ""}`;
}

export function Suggestions() {
  const { t, language } = useSettings();
  const [asking, setAsking] = useState<Found | null>(null);
  const shelves = useAsked(
    (signal) => requestsApi.suggestions(language, signal),
    [language],
    `requests:suggestions:${language}`,
  );

  return (
    <>
      {(shelves.answer ?? []).map((shelf) => (
        <section className="section" key={`${shelf.catalogue}:${shelf.genre_id ?? "popular"}`}>
          <RowHead
            title={shelf.genre ? t("requests.suggested_genre", { genre: shelf.genre }) : t("requests.suggested_popular")}
            to={moreAddress(shelf.catalogue, shelf.genre_id, shelf.genre)}
          >
            <span className="request-kind">{t(`requests.catalogue.${shelf.catalogue}`)}</span>
          </RowHead>
          <div className="request-grid request-two-lines">
            {shelf.items.map((one) => (
              <Answer key={`${one.catalogue}:${one.tmdb_id}`} found={one} onAsk={() => setAsking(one)} />
            ))}
          </div>
        </section>
      ))}
      {asking && <AskDialog found={asking} onClose={() => setAsking(null)} />}
    </>
  );
}
