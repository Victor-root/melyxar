/*
 * A grid of a whole library, with what narrows it.
 */

import { useEffect, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import { api } from "../api";
import type { Library } from "../api";
import { Card } from "../components/card";
import { Grid } from "../components/grid";
import { Picker } from "../components/panel";
import { Selecting } from "../components/selection";
import { ArrowRightIcon, CloseIcon, IdentifyIcon } from "../icons";
import { letterOfTheTopRow } from "../letters";
import { cardShapeOf, nameOfKind } from "../libraries";
import { ORDERS, useBrowsing } from "../screens/browsing";
import { useSettings } from "../settings";

export function LibraryPage({ libraries }: { libraries: Library[] }) {
  const { t } = useSettings();
  const { narrowing, choose, cards, more, loadMore, reach, loading, failed, filters } =
    useBrowsing();
  const { order, descending, genre, decade, search, unidentified, favourites } = narrowing;
  const library = libraries.find((entry) => entry.id === narrowing.library);
  const shape = cardShapeOf(library?.kind ?? narrowing.kind);
  /* What somebody filmed themselves is never waiting for a name, so there is
     nothing to narrow to. */
  const awaitsNames = library?.kind !== "home_media" && narrowing.kind !== "home_media";

  /*
   * A letter takes the grid to where its titles begin, with the row holding
   * the first of them at the top of the screen, and every other title still
   * there above and below it. It used to keep only that letter's titles,
   * which hid the rest of the library behind a press.
   *
   * The server says which work comes first under the letter, in the order
   * and inside the filters the grid is read with, since it is the one that
   * knows how a title is filed; the grid reads on until it holds that work,
   * and the page is scrolled to it once it is drawn.
   */
  const holder = useRef<HTMLDivElement>(null);
  const [jumping, setJumping] = useState<string | null>(null);
  const [landing, setLanding] = useState<{ work: string; letter: string } | null>(null);
  /* The letter of the titles at the top of the screen, lit on the rail. */
  const [reading, setReading] = useState<string | null>(null);
  /* The letter just jumped to, and where the page stood once it got there.
     Its row may open on the last titles of the letter before, and it is
     still the letter somebody asked for; it holds until the page is moved
     by hand. */
  const held = useRef<{ letter: string; at: number } | null>(null);
  const showsLetters = !!filters && filters.initials.length > 1 && order === "title";
  /* Only a library opened as such has letters, read by title: the favourites
     and a search have none. */
  const { id: opened } = useParams();
  const keepsRoomForLetters = opened !== undefined && order === "title";

  const jumpTo = (letter: string) => {
    setJumping(letter);
    api
      .works({ ...narrowing, initial: letter, limit: 1 })
      .then(async (page) => {
        const first = page.cards[0]?.id;
        if (first && (await reach(first))) {
          setLanding({ work: first, letter });
        }
      })
      .catch(() => {
        // The letter stays where it was, which is what a press that could
        // not be answered looks like.
      })
      .finally(() => setJumping(null));
  };

  useEffect(() => {
    if (!landing) {
      return;
    }
    const { work, letter } = landing;
    const target = holder.current?.querySelector<HTMLElement>(
      `[data-card="${CSS.escape(work)}"]`,
    );
    // Not drawn yet: the cards that hold it are on their way to the screen,
    // and this runs again when they arrive.
    if (!target) {
      return;
    }
    const box = scrollerOf(target);
    if (!box) {
      setLanding(null);
      return;
    }
    const style = getComputedStyle(box);
    const air = parseFloat(style.getPropertyValue("--gap-wide")) || 0;
    const below = () => target.getBoundingClientRect().top - box.getBoundingClientRect().top;
    /* Under the bar at the top while it is out, at the very top once it has
       stepped aside. Which one it is depends on the jump itself, since the
       bar steps aside when the page is read down and comes back when it is
       read up, so it is read again on every frame rather than guessed. */
    const clearOfTheBar = () => parseFloat(style.getPropertyValue("--header-room")) || 0;
    /* Straight there rather than gliding, and put there again on every
       frame until it holds still. The cards off the screen are not drawn and
       stand at a guessed height, so where the row is only settles as the
       cards around it are drawn: a glide drew them one after the other on
       the way and ended short of it, and a single second look came before
       they had been. */
    let frames = 0;
    let still = 0;
    let next = 0;
    const land = () => {
      const off = below() - clearOfTheBar() - air;
      if (Math.abs(off) > 1) {
        box.scrollBy({ top: off, behavior: "instant" });
        still = 0;
      } else {
        still += 1;
      }
      frames += 1;
      if (still < STILL_FRAMES && frames < LANDING_FRAMES) {
        next = requestAnimationFrame(land);
      } else {
        held.current = { letter, at: box.scrollTop };
        setReading(letter);
        setLanding(null);
      }
    };
    land();
    return () => cancelAnimationFrame(next);
  }, [landing, cards]);

  /* Which letter is on the screen, read again as the page moves: the one
     most of the top row is filed under. */
  useEffect(() => {
    const grid = holder.current;
    const box = grid ? scrollerOf(grid) : null;
    if (!showsLetters || !grid || !box) {
      return;
    }
    const filedUnder = new Map(cards.map((card) => [card.id, card.initial]));
    let asked = 0;
    const read = () => {
      asked = 0;
      const jumped = held.current;
      if (jumped && Math.abs(box.scrollTop - jumped.at) <= A_NUDGE) {
        setReading(jumped.letter);
        return;
      }
      held.current = null;
      setReading(letterAtTheTop(grid, box, filedUnder));
    };
    const soon = () => {
      if (!asked) {
        asked = requestAnimationFrame(read);
      }
    };
    read();
    box.addEventListener("scroll", soon, { passive: true });
    return () => {
      box.removeEventListener("scroll", soon);
      cancelAnimationFrame(asked);
    };
  }, [cards, showsLetters]);
  const lit = jumping ?? reading;

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
      {/* The room for the letters is kept from the first drawing of a grid
          read by title, before the server has said which letters there are:
          taken only once it had, every card shrank at once as the page
          arrived. */}
      <div
        className={`grid-with-letters${keepsRoomForLetters ? " grid-with-letters-kept" : ""}`}
        ref={holder}
      >
        <Selecting items={cards}>
          <Grid onReachEnd={loadMore} hasMore={more} shape={shape}>
            {cards.map((card) => (
              <Card key={card.id} card={card} shape={shape} />
            ))}
          </Grid>
        </Selecting>

        {/* Only the letters the library really has: a letter leading
            nowhere reads as a fault. One letter alone is no choice, and a
            grid read by year or by rating is not in the order of its
            letters, so there is nowhere for one to lead. */}
        {showsLetters && (
          <nav
            className="letters"
            aria-label={t("library.letters")}
            style={{ ["--letters" as string]: filters.initials.length }}
          >
            {filters.initials.map((entry) => (
              <button
                key={entry.name}
                type="button"
                className={`letter${lit === entry.name ? " letter-on" : ""}`}
                onClick={() => jumpTo(entry.name)}
                title={t("library.count", { count: entry.works })}
                aria-current={lit === entry.name ? "location" : undefined}
              >
                {entry.name.toUpperCase()}
              </button>
            ))}
          </nav>
        )}
      </div>

      {/* Said only while there is nothing on the screen yet. Once there is,
          the next cards are fetched well before the end comes into view, and
          a line under the grid about it, or about there being no more, only
          stood in the way. */}
      {loading && cards.length === 0 && <p className="notice">{t("library.loading")}</p>}
    </main>
  );
}

/** How many frames in a row a jump has to stand where it was sent before it
 *  counts as there: enough for the bar at the top to have answered it. */
const STILL_FRAMES = 6;
/** The most frames a jump is given to settle, under a second: long enough for
 *  the cards around it to be drawn, and never a page that fights a hand. */
const LANDING_FRAMES = 45;

/** How far the page may move after a jump before the letter jumped to gives
 *  way to the one on the screen: less than any hand moves it. */
const A_NUDGE = 2;

/** The letter of the titles at the top of the grid, where the bar at the top
 *  of the screen stops. */
function letterAtTheTop(
  grid: HTMLElement,
  box: HTMLElement,
  filedUnder: Map<string, string>,
): string | null {
  const room = parseFloat(getComputedStyle(box).getPropertyValue("--header-room")) || 0;
  const drawn = grid.querySelectorAll<HTMLElement>("[data-card]");
  return letterOfTheTopRow(
    drawn.length,
    (index) => {
      const { top, height } = drawn[index].getBoundingClientRect();
      return { top, height, letter: filedUnder.get(drawn[index].dataset.card ?? "") };
    },
    box.getBoundingClientRect().top + room,
  );
}

/** The box a page scrolls in, which is not the window here: the nearest one
 *  above the element that can be scrolled up and down. */
function scrollerOf(element: HTMLElement): HTMLElement | null {
  for (let box = element.parentElement; box; box = box.parentElement) {
    const { overflowY } = getComputedStyle(box);
    if (overflowY === "auto" || overflowY === "scroll") {
      return box;
    }
  }
  return null;
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
