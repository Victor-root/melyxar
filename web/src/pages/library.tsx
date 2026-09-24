/*
 * A grid of a whole library, with what narrows it.
 */

import type { Library } from "../api";
import { Card } from "../components/card";
import { Grid } from "../components/grid";
import { Selecting } from "../components/selection";
import { cardShapeOf, nameOfKind } from "../libraries";
import { ORDERS, useBrowsing } from "../screens/browsing";
import { useSettings } from "../settings";

export function LibraryPage({ libraries }: { libraries: Library[] }) {
  const { t } = useSettings();
  const { narrowing, choose, cards, more, loadMore, loading, failed, filters } = useBrowsing();
  const { order, descending, genre, decade, search, unidentified, initial, favourites } =
    narrowing;
  const library = libraries.find((entry) => entry.id === narrowing.library);
  const shape = cardShapeOf(library?.kind ?? narrowing.kind);
  /* What somebody filmed themselves is never waiting for a name, so there is
     nothing to narrow to. */
  const awaitsNames = library?.kind !== "home_media" && narrowing.kind !== "home_media";

  return (
    <main className="page">
      <div className="section-head">
        {/* A grid narrowed to a kind is that category, and says so: reaching
            it from the band and being told "All" reads as a wrong turn. */}
        <h1>
          {favourites
            ? t("nav.favourites")
            : (library?.name ??
              (narrowing.kind ? nameOfKind(narrowing.kind, libraries, t) : t("library.all")))}
        </h1>
        {/* What the library holds, not what has been scrolled to so far: a
            grid that counts its own loaded cards tells the viewer how far
            they have scrolled, which nobody asked. */}
        {library && !search && !genre && decade === undefined && !unidentified && !favourites && (
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

        {awaitsNames && (
          <button
            className={`toggle ${unidentified ? "toggle-on" : ""}`}
            onClick={() => choose("unidentified", unidentified ? null : "true")}
            aria-pressed={unidentified}
          >
            {t("library.filter.unidentified")}
          </button>
        )}
      </div>

      {failed && <p className="notice">{t("error.unreachable")}</p>}
      {!failed && cards.length === 0 && !loading && (
        <p className="notice">{t(favourites ? "favourites.empty" : "library.empty")}</p>
      )}

      {/* The grid and the letters beside it. A few hundred films is too long
          to scroll through and too short to search by hand every time, and the
          letter is the one thing anybody remembers about a title. */}
      <div className="grid-with-letters">
        <Selecting items={cards}>
          <Grid onReachEnd={loadMore} hasMore={more} shape={shape}>
            {cards.map((card) => (
              <Card key={card.id} card={card} shape={shape} />
            ))}
          </Grid>
        </Selecting>

        {/* Only the letters the library really has: a letter leading to an
            empty grid reads as a fault. One letter alone is no choice. */}
        {filters && filters.initials.length > 1 && (
          <nav className="letters" aria-label={t("library.letters")}>
            <button
              className={`letter ${initial ? "" : "letter-on"}`}
              onClick={() => choose("initial", null)}
            >
              {t("library.letters.all")}
            </button>
            {filters.initials.map((entry) => (
              <button
                key={entry.name}
                className={`letter ${initial === entry.name ? "letter-on" : ""}`}
                onClick={() => choose("initial", entry.name)}
                title={t("library.count", { count: entry.works })}
                aria-pressed={initial === entry.name}
              >
                {entry.name.toUpperCase()}
              </button>
            ))}
          </nav>
        )}
      </div>

      {loading && <p className="notice">{t("library.loading")}</p>}
      {!loading && !more && cards.length > 0 && (
        <p className="notice notice-faint">{t("library.end")}</p>
      )}
    </main>
  );
}
