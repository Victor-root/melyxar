/*
 * A grid of a whole library, with what narrows it.
 *
 * The choices live in the address rather than in memory, so a sorted, filtered
 * grid can be kept as a link, reloaded, and walked back to with the browser's
 * own back button.
 */

import { useCallback, useEffect, useState } from "react";
import { useParams, useSearchParams } from "react-router-dom";
import { api } from "../api";
import type { Card as CardData, Filters, Library } from "../api";
import { Card } from "../components/card";
import { Grid } from "../components/grid";
import { useSettings } from "../settings";

const ORDERS = ["title", "added_at", "release_year", "community_rating", "runtime"] as const;

export function LibraryPage({ libraries }: { libraries: Library[] }) {
  const { id } = useParams();
  const [parameters, setParameters] = useSearchParams();
  const { t } = useSettings();

  const [cards, setCards] = useState<CardData[]>([]);
  const [next, setNext] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [failed, setFailed] = useState(false);
  const [filters, setFilters] = useState<Filters | null>(null);

  const order = parameters.get("order") ?? "title";
  const descending = parameters.get("descending") === "true";
  const genre = parameters.get("genre") ?? undefined;
  const decade = parameters.get("decade") ? Number(parameters.get("decade")) : undefined;
  const search = parameters.get("search") ?? undefined;
  const unidentified = parameters.get("unidentified") === "true";
  const library = libraries.find((entry) => entry.id === id);

  // Any change to the choices starts the grid again from the top, since the
  // page after the fiftieth card of one ordering means nothing in another.
  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setFailed(false);
    api
      .works(
        { library: id, order, descending, genre, decade, search, unidentified },
        controller.signal,
      )
      .then((page) => {
        setCards(page.cards);
        setNext(page.next);
        setLoading(false);
      })
      .catch((error) => {
        if (!(error instanceof DOMException)) {
          setFailed(true);
          setLoading(false);
        }
      });
    return () => controller.abort();
  }, [id, order, descending, genre, decade, search, unidentified]);

  useEffect(() => {
    if (!id) {
      setFilters(null);
      return;
    }
    const controller = new AbortController();
    api
      .filters(id, controller.signal)
      .then(setFilters)
      .catch(() => setFilters(null));
    return () => controller.abort();
  }, [id]);

  const loadMore = useCallback(() => {
    if (!next || loading) {
      return;
    }
    setLoading(true);
    api
      .works({ library: id, order, descending, genre, decade, search, unidentified, after: next })
      .then((page) => {
        // Appended rather than replaced: the cards already on screen stay
        // where they are, which is what keeps the scroll position honest.
        setCards((current) => [...current, ...page.cards]);
        setNext(page.next);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, [next, loading, id, order, descending, genre, decade, search, unidentified]);

  const choose = (name: string, value: string | null) => {
    const updated = new URLSearchParams(parameters);
    if (value === null || value === "") {
      updated.delete(name);
    } else {
      updated.set(name, value);
    }
    setParameters(updated, { replace: true });
  };

  return (
    <main className="page">
      <div className="section-head">
        <h1>{library?.name ?? t("library.all")}</h1>
        {/* What the library holds, not what has been scrolled to so far: a
            grid that counts its own loaded cards tells the viewer how far
            they have scrolled, which nobody asked. */}
        {library && !search && !genre && decade === undefined && !unidentified && (
          <span className="count">{t("library.count", { count: library.works })}</span>
        )}
      </div>

      <div className="controls">
        <label className="choice">
          <span className="choice-label">{t("library.sort")}</span>
          <select value={order} onChange={(event) => choose("order", event.target.value)}>
            {ORDERS.map((value) => (
              <option key={value} value={value}>
                {t(`library.sort.${value}`)}
              </option>
            ))}
          </select>
        </label>

        <button
          className={`toggle ${descending ? "toggle-on" : ""}`}
          onClick={() => choose("descending", descending ? null : "true")}
          aria-pressed={descending}
        >
          {t("library.descending")}
        </button>

        {filters && filters.genres.length > 0 && (
          <label className="choice">
            <span className="choice-label">{t("library.filter.genre")}</span>
            <select
              value={genre ?? ""}
              onChange={(event) => choose("genre", event.target.value || null)}
            >
              <option value="">{t("library.filter.any")}</option>
              {filters.genres.map((entry) => (
                <option key={entry.name} value={entry.name}>
                  {entry.name} ({entry.works})
                </option>
              ))}
            </select>
          </label>
        )}

        {filters && filters.decades.length > 0 && (
          <label className="choice">
            <span className="choice-label">{t("library.filter.decade")}</span>
            <select
              value={decade ?? ""}
              onChange={(event) => choose("decade", event.target.value || null)}
            >
              <option value="">{t("library.filter.any")}</option>
              {filters.decades.map((entry) => (
                <option key={entry.decade} value={entry.decade}>
                  {entry.decade}s ({entry.works})
                </option>
              ))}
            </select>
          </label>
        )}

        <button
          className={`toggle ${unidentified ? "toggle-on" : ""}`}
          onClick={() => choose("unidentified", unidentified ? null : "true")}
          aria-pressed={unidentified}
        >
          {t("library.filter.unidentified")}
        </button>
      </div>

      {failed && <p className="notice">{t("error.unreachable")}</p>}
      {!failed && cards.length === 0 && !loading && (
        <p className="notice">{t("library.empty")}</p>
      )}

      <Grid onReachEnd={loadMore} hasMore={next !== null}>
        {cards.map((card) => (
          <Card key={card.id} card={card} />
        ))}
      </Grid>

      {loading && <p className="notice">{t("library.loading")}</p>}
      {!loading && next === null && cards.length > 0 && (
        <p className="notice notice-faint">{t("library.end")}</p>
      )}
    </main>
  );
}
