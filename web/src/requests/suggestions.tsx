/*
 * What the page of requests shows before anything is typed: rows of titles
 * taken from the genres this account watches most, or the most popular ones
 * while it has not watched enough to tell. Each is asked for the same way as
 * an answer of a search.
 */

import { useState } from "react";
import { useAsked } from "../asking";
import { Row, RowHead } from "../components/row";
import { useSettings } from "../settings";
import { Answer } from "./answer";
import { requestsApi } from "./api";
import type { Found } from "./api";
import { AskDialog } from "./ask";

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
        <section className="section" key={`${shelf.catalogue ?? "all"}:${shelf.genre ?? "popular"}`}>
          <RowHead
            title={shelf.genre ? t("requests.suggested_genre", { genre: shelf.genre }) : t("requests.suggested_popular")}
          >
            {shelf.catalogue && <span className="request-kind">{t(`requests.catalogue.${shelf.catalogue}`)}</span>}
          </RowHead>
          <Row>
            {shelf.items.map((one) => (
              <div className="request-shelf-card" data-card key={`${one.catalogue}:${one.tmdb_id}`}>
                <Answer found={one} onAsk={() => setAsking(one)} />
              </div>
            ))}
          </Row>
        </section>
      ))}
      {asking && <AskDialog found={asking} onClose={() => setAsking(null)} />}
    </>
  );
}
