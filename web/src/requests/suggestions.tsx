/*
 * What the page of requests shows before anything is typed: two lines of
 * titles for each of the genres this account watches most, or for the most
 * popular ones while it has not watched enough to tell, and under each the
 * way to many more. Each is asked for the same way as an answer of a search.
 */

import { useLayoutEffect, useRef, useState } from "react";
import { useAsked } from "../asking";
import { RowHead } from "../components/row";
import { useSettings } from "../settings";
import { Answer } from "./answer";
import { requestsApi } from "./api";
import type { Catalogue, Found } from "./api";
import { AskDialog } from "./ask";

/** How many lines of tiles a shelf shows. */
const LINES = 2;

/** How many tiles stand on a line of the grid, read from the grid itself so
 *  that a shelf shows two whole lines whatever the width. */
function useTilesPerLine(grid: React.RefObject<HTMLDivElement | null>): number {
  const [tiles, setTiles] = useState(1);
  useLayoutEffect(() => {
    const element = grid.current;
    if (!element) {
      return;
    }
    const read = () => setTiles(Math.max(1, getComputedStyle(element).gridTemplateColumns.split(" ").length));
    read();
    const watcher = new ResizeObserver(read);
    watcher.observe(element);
    return () => watcher.disconnect();
  }, [grid]);
  return tiles;
}

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

function ShelfGrid({ items, onAsk }: { items: Found[]; onAsk: (found: Found) => void }) {
  const grid = useRef<HTMLDivElement>(null);
  const tiles = useTilesPerLine(grid);
  return (
    <div ref={grid} className="request-grid">
      {items.slice(0, tiles * LINES).map((one) => (
        <Answer key={`${one.catalogue}:${one.tmdb_id}`} found={one} onAsk={() => onAsk(one)} />
      ))}
    </div>
  );
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
          <ShelfGrid items={shelf.items} onAsk={setAsking} />
        </section>
      ))}
      {asking && <AskDialog found={asking} onClose={() => setAsking(null)} />}
    </>
  );
}
