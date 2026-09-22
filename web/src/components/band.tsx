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
 * other card here does; a library of fewer than five fans out however many
 * it has, in an arrangement written for that many rather than a full fan
 * with holes in it; and a library with nothing in it at all shows its own
 * mark, faint, where the posters will stand once there are any.
 */

import { Link } from "react-router-dom";
import type { Card, Home, Library } from "../api";
import { useShownPicture } from "./picture";
import { whereAKindLeads, worksOfKind } from "../libraries";
import { useSettings } from "../settings";
import { ChevronRightIcon, KindIcon } from "../icons";

type Shelf = Home["shelves"][number];

/** How many posters a tile fans out at its widest. Five reaches the sides of
 *  a tile this shape, where three left a hole at either end, and the
 *  stylesheet draws anything from one to five: the day the mobile pass wants
 *  three again it asks for three, and the rules for three are already here. */
const IN_A_FAN = 5;

/**
 * How much room one poster of the fan really has, for the browser to pick a
 * size with.
 *
 * Written out in plain lengths, as the cards' own are: this attribute is read
 * before the page has a stylesheet, so a `var()` in here is not a length the
 * browser can use, the whole thing is thrown away, and every poster is then
 * asked for at the full width of the window.
 *
 * A poster of the fan is drawn at around eighty points across at the widest
 * a tile ever gets. What is asked for here is more than that on purpose: it
 * is the value that lands on the same stored width the row of that kind is
 * already loading further down the page, on an ordinary screen and on a fine
 * one alike, so the fan costs no second file. Asking for exactly what is
 * drawn would save nothing and fetch a size nothing else uses.
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
 * whose newest arrivals have not been looked up yet still has a full fan to
 * show, and showing its newest holes instead would say the library is empty
 * when it is the opposite.
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
          under it, and a screen reader reading out five film titles nobody
          asked for is five titles in the way of the one word that matters.

          The fan is drawn twice: once standing, once upside down under its
          own feet and faded out, which is the floor it stands on. One
          component for both, so the two can never fall out of step, and the
          same pictures either way, so the floor costs nothing to fetch. */}
      {fan.length > 0 ? (
        <>
          {/* The kind's mark, run off the top corner rather than stood
              behind the middle of the fan, where its ends poked past the
              posters covering it and read as something broken. A library
              with nothing in it gets the centred one below instead, which
              is whole, so the two never appear together. */}
          <span className="band-ghost" aria-hidden="true">
            <KindIcon kind={shelf.kind} />
          </span>
          <span className="band-halo" aria-hidden="true" />
          <Fan fan={fan} mirror />
          <Fan fan={fan} />
        </>
      ) : (
        <span className="band-mark" aria-hidden="true">
          <KindIcon kind={shelf.kind} />
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
 * The fan itself, standing or reflected.
 *
 * The same component both times so the floor can never drift from what is
 * standing on it: the reflection is not a second arrangement but the same
 * one turned over, which the stylesheet does with a flip about the line the
 * posters stand on. What the mirror adds is only that flip and the fade.
 */
function Fan({ fan, mirror }: { fan: Card[]; mirror?: boolean }) {
  return (
    <span
      className={`band-posters band-fan-${fan.length}${mirror ? " band-mirror" : ""}`}
      aria-hidden="true"
    >
      {fan.map((card) => (
        <FanPoster key={card.id} card={card} />
      ))}
    </span>
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
