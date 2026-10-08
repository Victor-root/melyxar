/*
 * A grid of a whole library, with what narrows it.
 */

import { BrowseBubble } from "../components/browse-bubble";
import { UploadButton } from "../components/upload-button";
import { useEffect, useMemo, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import { api } from "../api";
import type { Library } from "../api";
import { Card } from "../components/card";
import { InOrder } from "../components/in-order";
import { LetterDrop } from "../components/letter-drop";
import type { DropHandle } from "../components/letter-drop";
import { useRailScrub } from "../components/rail-scrub";
import { useAccount } from "../account";
import { PHONE, useMediaQuery } from "../media-query";
import { Grid } from "../components/grid";
import { Picker } from "../components/panel";
import { Selecting } from "../components/selection";
import { ArrowRightIcon, ChevronRightIcon, CloseIcon, IdentifyIcon } from "../icons";
import { useMarks } from "../marks";
import { barRoom } from "../bar-room";
import { landOn, scrollerOf } from "../landing";
import { letterOfTheTopRow } from "../letters";
import { cardShapeOf, nameOfKind } from "../libraries";
import { ORDERS, useBrowsing } from "../screens/browsing";
import { useSettings } from "../settings";
import { useTabPage } from "../tab-page";

export function LibraryPage({
  libraries,
  besides,
  besidesFound = false,
}: {
  libraries: Library[];
  /** What else a search found, drawn above the grid. */
  besides?: React.ReactNode;
  /** Whether that found anything, so an empty grid is not called empty. */
  besidesFound?: boolean;
}) {
  const { t } = useSettings();
  const { account } = useAccount();
  const phone = useMediaQuery(PHONE);
  const { narrowing, choose, cards: read, more, loadMore, reach, loading, failed, filters } =
    useBrowsing();
  const { order, descending, genre, decade, search, unidentified, favourites, watchLater } =
    narrowing;
  /* A view of what this account marked holds what is marked now, not what
     was marked when the server was asked: a card taken off it here, or by
     being watched, leaves at once rather than at the next reading. Every
     other grid holds what the server read, whatever its marks. */
  const marks = useMarks();
  const keeps = favourites ? marks.favouriteOf : watchLater ? marks.watchLaterOf : null;
  /* A card deleted from here leaves at once as well, and leaves the grid
     rather than a hole in it. The same list when nothing left, so the grid
     is not drawn again for a mark that took nothing out of it. */
  const { goneOf } = marks;
  const cards = useMemo(() => {
    const kept = read.filter((card) => !goneOf(card.id) && (keeps === null || keeps(card)));
    return kept.length === read.length ? read : kept;
  }, [read, keeps, goneOf]);
  const library = libraries.find((entry) => entry.id === narrowing.library);
  const shape = cardShapeOf(library?.kind ?? narrowing.kind);
  useTabPage(
    favourites
      ? t("nav.favourites")
      : watchLater
        ? t("nav.watch_later")
        : library
          ? library.name
          : narrowing.kind
            ? nameOfKind(narrowing.kind, libraries, t)
            : search
              ? t("nav.search")
              : null,
  );
  /* What somebody filmed themselves is never waiting for a name, so there is
     nothing to narrow to. */
  const awaitsNames = library?.kind !== "home_media" && narrowing.kind !== "home_media";
  /* In a library, offered only while something there still waits for a
     name, and kept while it is on, so the last one named does not take the
     way back out with it. A grid over several libraries has no count. */
  const offersUnidentified =
    awaitsNames &&
    (unidentified || !library || (filters?.awaiting_identification ?? 0) > 0);

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
  /* Where the letter last jumped to begins, marked in the grid itself: its
     row may open on the last titles of the letter before. Kept until another
     letter is chosen or the grid is read another way. */
  const [starts, setStarts] = useState<{ work: string; letter: string } | null>(null);
  useEffect(
    () => setStarts(null),
    [order, descending, genre, decade, search, unidentified, narrowing.library, narrowing.kind],
  );
  /* The letter of the titles at the top of the screen, lit on the rail. */
  const [reading, setReading] = useState<string | null>(null);
  /* The letter just jumped to, and where the page stood once it got there.
     Its row may open on the last titles of the letter before, and it is
     still the letter somebody asked for; it holds until the page is moved
     by hand. */
  const held = useRef<{ letter: string; at: number } | null>(null);
  const showsLetters = !!filters && filters.initials.length > 1 && order === "title";
  /* Only a library opened as such has letters, read by title: the favourites,
     what is put aside for later and a search have none. */
  const { id: opened } = useParams();
  const keepsRoomForLetters = opened !== undefined && order === "title";

  const jumpTo = (letter: string) => {
    setJumping(letter);
    api
      .works({ ...narrowing, initial: letter, limit: 1 })
      .then(async (page) => {
        const first = page.cards[0]?.id;
        if (first && (await reach(first))) {
          setStarts({ work: first, letter });
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
      `[data-starts="${CSS.escape(work)}"], [data-card="${CSS.escape(work)}"]`,
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
    return landOn(target, box, (at) => {
      held.current = { letter, at };
      setReading(letter);
      setLanding(null);
    });
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
    /* The cards drawn, looked for again only once a card has come or gone:
       looked for on every frame of a scroll, the thousands of a library were
       walked through each time. The buttons a card makes under the pointer
       come and go too, and say nothing about which cards there are. */
    let drawn: NodeListOf<HTMLElement> | null = null;
    const isACard = (node: Node) =>
      node instanceof HTMLElement && (node.matches("[data-card]") || node.querySelector("[data-card]") !== null);
    const changed = new MutationObserver((records) => {
      if (records.some((record) => [...record.addedNodes, ...record.removedNodes].some(isACard))) {
        drawn = null;
      }
    });
    changed.observe(grid, { childList: true, subtree: true });
    let asked = 0;
    const read = () => {
      asked = 0;
      const jumped = held.current;
      if (jumped && Math.abs(box.scrollTop - jumped.at) <= A_NUDGE) {
        setReading(jumped.letter);
        return;
      }
      held.current = null;
      drawn ??= grid.querySelectorAll<HTMLElement>("[data-card]");
      setReading(letterAtTheTop(drawn, box, filedUnder));
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
      changed.disconnect();
    };
  }, [cards, showsLetters]);
  const lit = jumping ?? reading;
  const drop = useRef<DropHandle>(null);
  const scrub = useRailScrub((index) => {
    if (filters) {
      jumpTo(filters.initials[index].name);
    }
  }, drop);

  /* The cards made once for what the grid holds, so the letter lit beside it
     changing as the page is scrolled redraws the letters and nothing else.
     Made again at every render, the few hundred cards of a library were all
     drawn again each time the top row reached another letter: measured,
     most of what the page did while it was being scrolled. */
  const drawn = useMemo(
    () =>
      cards.flatMap((card) => [
        ...(starts?.work === card.id
          ? [
              <div
                key={`starts:${card.id}`}
                className={`letter-starts letter-starts-${shape}`}
                data-starts={card.id}
                aria-hidden="true"
              >
                <span>{starts.letter.toUpperCase()}</span>
                <ChevronRightIcon size={40} />
              </div>,
            ]
          : []),
        <Card key={card.id} card={card} shape={shape} />,
      ]),
    [cards, shape, starts],
  );

  const fields = (
    <>
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

      {/* On a phone the way to put files in is in the piece with the
          sort, as the music libraries have it, rather than a piece of
          its own under it. */}
      {library && phone && account?.may_upload && (
        <span className="browse-field music-library-tools">
          <UploadButton library={library} bare className="music-tab music-play-tool" />
        </span>
      )}
    </>
  );

  /* A press of the same kind as the others in the bubble of a phone, a piece
     of its own beside the bar elsewhere. */
  const unidentifiedPress = offersUnidentified && (
    <button
      type="button"
      className={
        phone
          ? `music-tab music-play-tool${unidentified ? " music-tab-on" : ""}`
          : `browse-piece browse-alone${unidentified ? " browse-alone-on" : ""}`
      }
      onClick={() => choose("unidentified", unidentified ? null : "true")}
      aria-pressed={unidentified}
      aria-label={t("library.filter.unidentified")}
      title={t("library.filter.unidentified")}
    >
      <IdentifyIcon size={16} />
      {/* Left out when the bar is short of room, the icon standing for it. */}
      <span className="browse-alone-words">{t("library.filter.unidentified")}</span>
    </button>
  );

  return (
    <main className={`page library-page${keepsRoomForLetters ? " page-beside-letters" : ""}`}>
      {/* What the grid is and how it is read, which on a wide screen rise
          together into the band of the bar at the top. */}
      <div className="browse-head">
        <div className="section-head">
          {/* A grid narrowed to a kind is that category, and says so: reaching
              it from the band and being told "All" reads as a wrong turn. */}
          <h1>
            {favourites
              ? t("nav.favourites")
              : watchLater
                ? t("nav.watch_later")
                : (library?.name ??
                (narrowing.kind ? nameOfKind(narrowing.kind, libraries, t) : t("library.all")))}
          </h1>
          {/* What the library holds, not what has been scrolled to so far: a
              grid that counts its own loaded cards tells the viewer how far
              they have scrolled, which nobody asked. */}
          {library && !search && !genre && decade === undefined && !unidentified && !favourites && !watchLater && (
            <span className="count">{t("library.count", { count: library.works })}</span>
          )}
        </div>

        {/* How the grid is read, in one piece of the glass the bar at the top is
            made of: a row of loose system fields was the one place left that
            looked like a form rather than like Melyxar. */}
        {phone ? (
          <BrowseBubble>
            {fields}
            {unidentifiedPress}
          </BrowseBubble>
        ) : (
          <div className="browse-bar">
            <div className="browse-piece">{fields}</div>
            {library && <UploadButton library={library} className="browse-piece browse-alone" />}
            {unidentifiedPress}
          </div>
        )}
      </div>

      {besides}

      {failed && <p className="notice">{t("error.unreachable")}</p>}
      {!failed && cards.length === 0 && !loading && !besidesFound && (
        <p className="notice">
          {t(favourites ? "favourites.empty" : watchLater ? "watch_later.empty" : "library.empty")}
        </p>
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
          <InOrder cards={cards}>
            <Grid onReachEnd={loadMore} hasMore={more} shape={shape}>
              {drawn}
            </Grid>
          </InOrder>
        </Selecting>

        {/* Only the letters the library really has: a letter leading
            nowhere reads as a fault. One letter alone is no choice, and a
            grid read by year or by rating is not in the order of its
            letters, so there is nowhere for one to lead. */}
        {showsLetters && (
          <>
          <nav
            className="letters"
            aria-label={t("library.letters")}
            style={{ ["--letters" as string]: filters.initials.length }}
            {...scrub.rail}
          >
            {filters.initials.map((entry, index) => (
              <button
                key={entry.name}
                type="button"
                className={`letter${lit === entry.name ? " letter-on" : ""}`}
                onClick={() => scrub.press(index)}
                title={t("library.count", { count: entry.works })}
                aria-current={lit === entry.name ? "location" : undefined}
              >
                {entry.name.toUpperCase()}
              </button>
            ))}
          </nav>
          <LetterDrop ref={drop} letters={filters.initials.map((entry) => entry.name.toUpperCase())} />
          </>
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

/** How far the page may move after a jump before the letter jumped to gives
 *  way to the one on the screen: less than any hand moves it. */
const A_NUDGE = 2;

/** The letter of the titles at the top of the grid, where the bar at the top
 *  of the screen stops. */
function letterAtTheTop(
  drawn: NodeListOf<HTMLElement>,
  box: HTMLElement,
  filedUnder: Map<string, string>,
): string | null {
  const room = barRoom();
  return letterOfTheTopRow(
    drawn.length,
    (index) => {
      const { top, height } = drawn[index].getBoundingClientRect();
      return { top, height, letter: filedUnder.get(drawn[index].dataset.card ?? "") };
    },
    box.getBoundingClientRect().top + room,
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
