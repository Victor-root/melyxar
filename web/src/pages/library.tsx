/*
 * A grid of a whole library, with what narrows it.
 */

import type { Library } from "../api";
import { Card } from "../components/card";
import { Grid } from "../components/grid";
import { Picker } from "../components/panel";
import { Selecting } from "../components/selection";
import { ArrowRightIcon, CloseIcon, IdentifyIcon } from "../icons";
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
      {/* What the grid is and how it is read, which on a wide screen rise
          together into the band of the bar at the top. */}
      <div className="browse-head">
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

        {/* How the grid is read, in one piece of the glass the bar at the top is
            made of: a row of loose system fields was the one place left that
            looked like a form rather than like Melyxar. */}
        <div className="browse-bar">
          <div className="browse-piece">
            <span className="browse-field">
              <span className="browse-label">{t("library.sort")}</span>
              <Picker
                value={order}
                options={ORDERS.map((value) => [value, t(`library.sort.${value}`)] as const)}
                onPick={(value) => choose("order", value)}
                label={t("library.sort")}
              />
              {/* Which way round, drawn as the way the arrow points rather than
                  written out: a sentence for an arrow's worth of meaning. */}
              <button
                type="button"
                className={`browse-direction${descending ? " browse-direction-down" : ""}`}
                onClick={() => choose("descending", descending ? null : "true")}
                aria-pressed={descending}
                aria-label={t("library.descending")}
                title={t(descending ? "library.descending" : "library.ascending")}
              >
                <ArrowRightIcon size={16} />
              </button>
            </span>

            {filters && filters.genres.length > 0 && (
              <Narrower
                label={t("library.filter.genre")}
                value={genre ?? ""}
                options={filters.genres.map((entry) => [
                  entry.name,
                  `${entry.name} (${entry.works})`,
                ])}
                onPick={(value) => choose("genre", value || null)}
              />
            )}

            {filters && filters.decades.length > 0 && (
              <Narrower
                label={t("library.filter.decade")}
                value={decade === undefined ? "" : String(decade)}
                options={filters.decades.map((entry) => [
                  String(entry.decade),
                  `${entry.decade}s (${entry.works})`,
                ])}
                onPick={(value) => choose("decade", value || null)}
              />
            )}
          </div>

          {awaitsNames && (
            <button
              type="button"
              className={`browse-piece browse-alone${unidentified ? " browse-alone-on" : ""}`}
              onClick={() => choose("unidentified", unidentified ? null : "true")}
              aria-pressed={unidentified}
              aria-label={t("library.filter.unidentified")}
              title={t("library.filter.unidentified")}
            >
              <IdentifyIcon size={16} />
              {/* Left out when the bar is short of room, the icon standing
                  for it. */}
              <span className="browse-alone-words">{t("library.filter.unidentified")}</span>
            </button>
          )}
        </div>
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

/**
 * One filter of the bar: what it narrows by, what it is set to, and, once set,
 * the way to take it off in one press rather than by finding "Any" in its list.
 */
function Narrower({
  label,
  value,
  options,
  onPick,
}: {
  label: string;
  /** Empty while nothing is narrowed. */
  value: string;
  options: (readonly [string, string])[];
  onPick: (value: string) => void;
}) {
  const { t } = useSettings();
  return (
    <span className={`browse-field${value ? " browse-field-on" : ""}`}>
      <span className="browse-label">{label}</span>
      <Picker
        value={value}
        options={[["", t("library.filter.any")], ...options]}
        onPick={onPick}
        label={label}
      />
      {value && (
        <button
          type="button"
          className="browse-clear"
          onClick={() => onPick("")}
          aria-label={t("library.filter.clear", { name: label })}
          title={t("library.filter.clear", { name: label })}
        >
          <CloseIcon size={14} />
        </button>
      )}
    </span>
  );
}
