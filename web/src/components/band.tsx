/*
 * The band of ways in, straight under the banner.
 *
 * A home page made of rows answers "what is there", and answers it well. What
 * it does not answer is "take me to the films", which is what somebody who
 * came here knowing what they wanted is looking for, and which used to mean
 * reading the bar at the top word by word.
 *
 * One wide tile per kind of library this server really holds, in the order
 * the bar offers them. Each one fans out a few real posters out of that
 * library rather than standing on one borrowed still: a still is one film
 * seen wide, and what a way in has to say is "this is a shelf of these".
 *
 * The posters are the ones the server already sent for the row of that kind
 * further down the page, in the order it sent them, which is the newest
 * first. So the same tile shows the same posters from one visit to the next,
 * and the browser is asked for no picture it is not already fetching anyway.
 *
 * What a library short of posters shows is said in three places rather than
 * left to chance: a work with none shows the letter it begins with, as every
 * other card here does; one or two works fan out as one or two rather than
 * as three with holes in; and a library with nothing in it at all shows its
 * own mark, faint, where the posters will stand once there are any.
 */

import { Link } from "react-router-dom";
import type { Card, Home, Library } from "../api";
import { useShownPicture } from "./picture";
import { whereAKindLeads, worksOfKind } from "../libraries";
import { useSettings } from "../settings";
import { ChevronRightIcon, KindIcon } from "../icons";

type Shelf = Home["shelves"][number];

/** How many posters a tile fans out at its widest. Three is what a tile this
 *  shape holds without any of them being a sliver, and the stylesheet draws
 *  one, two or three from the same set of rules. */
const IN_A_FAN = 3;

/**
 * How much room one poster of the fan really has, for the browser to pick a
 * size with.
 *
 * Written out in plain lengths, as the cards' own are: this attribute is read
 * before the page has a stylesheet, so a `var()` in here is not a length the
 * browser can use, the whole thing is thrown away, and every poster is then
 * asked for at the full width of the window.
 *
 * The centre poster stands at about two thirds of a tile's height and a tile
 * is at most four hundred and twenty points across, so a hundred and thirty
 * covers it with room to spare. The screen's own fineness is the browser's
 * business: a fine screen takes the next size up by itself, which is the same
 * one the row of that kind further down the page is already loading.
 */
const ROOM_FOR_A_POSTER = "(max-width: 900px) 22vw, 130px";

export function Band({ shelves, libraries }: { shelves: Shelf[]; libraries: Library[] }) {
  const { t } = useSettings();

  if (shelves.length === 0) {
    return null;
  }
  return (
    <nav className="band" aria-label={t("home.band")}>
      {shelves.map((shelf) => (
        <Tile key={shelf.kind} shelf={shelf} libraries={libraries} />
      ))}
    </nav>
  );
}

/**
 * The works a tile fans out, out of everything the server sent for that kind.
 *
 * The newest first, which is the order it arrived in, and the ones that have
 * a poster ahead of the ones that have none: a library of three hundred films
 * whose newest arrival has not been looked up yet still has three posters to
 * show, and showing its three newest holes instead would say the library is
 * empty when it is the opposite.
 *
 * What is left over, on a library where nothing has a poster at all, is the
 * newest works with no poster, which the fan draws as the letter each one
 * begins with. That is the same answer every card in this interface gives to
 * a work with no picture, rather than a fourth thing to learn.
 */
export function fanOf(cards: Card[]): Card[] {
  const drawn = cards.filter((card) => card.poster.length > 0);
  const bare = cards.filter((card) => card.poster.length === 0);
  return [...drawn, ...bare].slice(0, IN_A_FAN);
}

function Tile({ shelf, libraries }: { shelf: Shelf; libraries: Library[] }) {
  const { t, language } = useSettings();
  const works = worksOfKind(shelf.kind, libraries);
  const fan = fanOf(shelf.cards);

  return (
    <Link
      className="band-tile"
      to={whereAKindLeads(shelf.kind, libraries)}
      style={{
        /* A breath of the newest work's own colour in the panel behind the
           fan, so two tiles side by side are not the same dark rectangle
           twice. The stylesheet decides how much of it survives. */
        ["--tile-color" as string]: shelf.cards[0]?.color ?? "var(--surface-raised)",
      }}
    >
      {/* Decorative through and through: the tile is named in words right
          under it, and a screen reader reading out three film titles nobody
          asked for is three titles in the way of the one word that matters.

          A library with nothing in it has nothing to fan out, and an empty
          box says nothing at all: it shows its own mark instead, faint and
          large, standing where the posters will stand once there are any. */}
      {fan.length > 0 ? (
        <span className={`band-posters band-fan-${fan.length}`} aria-hidden="true">
          {fan.map((card) => (
            <FanPoster key={card.id} card={card} />
          ))}
        </span>
      ) : (
        <span className="band-empty" aria-hidden="true">
          <KindIcon kind={shelf.kind} size={72} />
        </span>
      )}

      <span className="band-words">
        <span className="band-name">
          <KindIcon kind={shelf.kind} size={17} />
          {t(`kind.${shelf.kind}`)}
        </span>
        {/* Grouped the way the language groups thousands: eighty three
            thousand written as one run of figures is a figure nobody reads. */}
        <span className="band-count">
          {t("home.band.count", { count: works.toLocaleString(language) })}
        </span>
      </span>
      <span className="band-arrow" aria-hidden="true">
        <ChevronRightIcon size={18} />
      </span>
    </Link>
  );
}

/**
 * One poster of the fan.
 *
 * Its own component because each one answers for itself: a poster the server
 * has a row for but no longer a file for is one poster falling back to its
 * letter, not a fan going out.
 */
function FanPoster({ card }: { card: Card }) {
  const { picture, itDidNotLoad } = useShownPicture(card.poster);

  return (
    <span
      className="band-poster"
      style={{ ["--card-color" as string]: card.color ?? "var(--surface-raised)" }}
    >
      {picture ? (
        <img
          src={picture.src}
          srcSet={picture.srcSet}
          sizes={ROOM_FOR_A_POSTER}
          alt=""
          loading="lazy"
          decoding="async"
          draggable={false}
          onError={itDidNotLoad}
        />
      ) : (
        <span className="band-poster-initial">{card.title.slice(0, 1)}</span>
      )}
    </span>
  );
}
