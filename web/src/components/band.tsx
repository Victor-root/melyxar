/*
 * The band of ways in, straight under the banner.
 *
 * A home page made of rows answers "what is there", and answers it well. What
 * it does not answer is "take me to the films", which is what somebody who
 * came here knowing what they wanted is looking for, and which used to mean
 * reading the bar at the top word by word.
 *
 * One wide tile per kind of library this server really holds, in the order
 * the bar offers them. Each carries a picture borrowed from the newest work
 * of that kind, so the band is a set of ways in rather than a row of named
 * rectangles; a server whose films nobody has looked up yet has no picture to
 * borrow, and the tile falls back to its own colour rather than to a hole.
 */

import { Link } from "react-router-dom";
import type { Home, Library } from "../api";
import { useShownPicture } from "./picture";
import { whereAKindLeads, worksOfKind } from "../libraries";
import { useSettings } from "../settings";
import { ChevronRightIcon, KindIcon } from "../icons";

type Shelf = Home["shelves"][number];

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

function Tile({ shelf, libraries }: { shelf: Shelf; libraries: Library[] }) {
  const { t, language } = useSettings();
  const { picture, framing, itDidNotLoad } = useShownPicture(shelf.picture);
  const works = worksOfKind(shelf.kind, libraries);

  return (
    <Link
      className="band-tile"
      to={whereAKindLeads(shelf.kind, libraries)}
      style={{
        ["--card-color" as string]: shelf.cards[0]?.color ?? "var(--surface-raised)",
      }}
    >
      {picture && (
        <img
          className="band-picture"
          src={picture.src}
          srcSet={picture.srcSet}
          sizes="(max-width: 900px) 50vw, 20vw"
          style={{ objectPosition: framing }}
          alt=""
          loading="lazy"
          decoding="async"
          onError={itDidNotLoad}
        />
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
