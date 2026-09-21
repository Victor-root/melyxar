/*
 * One card in a grid or a row.
 *
 * The colour of the poster is painted behind it before the picture arrives,
 * so a grid has colour from the first moment instead of a wall of grey holes.
 * A film nobody recognised keeps its place and wears a marker: a file set
 * aside is a file forgotten.
 *
 * What the hover offers is the whole ergonomics of this interface: play in
 * the middle, watched at the top right, liked and the rest at the bottom
 * right. None of it grows the card or moves its neighbours, because a grid
 * that reflows under the pointer is a grid nobody can aim at.
 *
 * And none of it is hover alone. A finger has no hover, so everything here is
 * reachable by a press on the card's own menu, and the menu opens on a tap
 * rather than on a pointer that never arrives. The mobile interface is a
 * later worksite; a card that could only be used with a mouse would be a
 * rewrite when it comes rather than an adjustment.
 */

import { useRef, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import type { Card as CardData } from "../api";
import { useMarks } from "../marks";
import { useSettings } from "../settings";
import { useShownPicture } from "./picture";
import { CardMenu } from "./cardmenu";
import { SeenMark } from "./seen";
import { HeartIcon, MoreIcon, PlayIcon } from "../icons";

/** How a card is laid out: standing like a poster, or lying like a still. */
export type CardShape = "standing" | "lying";

export function Card({
  card,
  shape = "standing",
  /** How far in, between nought and one, when a row knows and the card does
      not: what somebody left halfway carries it in the row's own answer. */
  watched,
  /** The name the card leads with, when it is not the work's own: an episode
      is shown under the name of its series, which is the only name anybody
      remembers. */
  lead,
  /** The faint line under it, in place of the year. */
  note,
  /** A few words at the right of the name, where a row of stills needs them:
      how much of it is left. */
  trailing,
}: {
  card: CardData;
  shape?: CardShape;
  watched?: number;
  lead?: string;
  note?: string;
  trailing?: string;
}) {
  const { t } = useSettings();
  const navigate = useNavigate();
  const marks = useMarks();
  /* A lying card is nearly twice as wide as it is tall and a poster is two
     thirds as wide as it is tall: filling one with the other cuts a band out
     of the middle of the picture. So such a row is given something wide, and
     falls back to the poster only where the server had nothing wide to
     send. */
  const { picture: poster, itDidNotLoad } = useShownPicture(
    shape === "lying" && card.wide.length > 0 ? card.wide : card.poster,
  );
  /* Where the menu is drawn from. Kept as the button's place at the moment
     it was pressed, because the menu is drawn over the page rather than
     inside the card, which clips what it holds. */
  const kebab = useRef<HTMLButtonElement>(null);
  const [menuFrom, setMenuFrom] = useState<DOMRect | null>(null);

  const unknown = card.identification === "unidentified" || card.identification === "pending";
  const seen = marks.seenOf(card);
  const favourite = marks.favouriteOf(card);
  /* A series is opened rather than played: what a play button on one would
     mean is the next episode, which is what the row of them is for. */
  const playable = card.source !== null && card.kind !== "series";
  const howFar =
    watched ??
    (card.resume_from_seconds !== null && card.runtime_minutes
      ? card.resume_from_seconds / (card.runtime_minutes * 60)
      : undefined);

  const stop = (doing: () => void) => (event: React.MouseEvent) => {
    // The card is one big link; everything drawn on top of it has to say so.
    event.preventDefault();
    event.stopPropagation();
    doing();
  };

  return (
    <article
      className={`card card-${shape}`}
      data-card
      style={{ ["--card-color" as string]: card.color ?? "var(--surface-raised)" }}
    >
      <div className="card-picture">
        {poster ? (
          <img
            src={poster.src}
            srcSet={poster.srcSet}
            sizes="(max-width: 700px) 40vw, var(--card-width)"
            alt=""
            loading="lazy"
            decoding="async"
            draggable={false}
            onError={itDidNotLoad}
          />
        ) : (
          <span className="card-initial" aria-hidden="true">
            {card.title.slice(0, 1)}
          </span>
        )}

        {/* The whole card leads to the work. Stretched over the picture
            rather than wrapped around everything, so the buttons drawn on top
            are buttons and not parts of a link. */}
        <Link
          className="card-open"
          to={`/work/${card.id}`}
          title={card.title}
          draggable={false}
        >
          <span className="visually-hidden">{card.title}</span>
        </Link>

        {/* The reason wins over the state: knowing a film is not identified is
            what the grid already showed, knowing why is what sends somebody to
            rename a file rather than to press the button again. */}
        {unknown && (
          <span className="card-flag">
            {card.identification_note
              ? t(`note.short.${card.identification_note}`)
              : t(card.identification === "pending" ? "work.pending" : "work.unidentified")}
          </span>
        )}

        {/* Where somebody is with it: how many episodes are left, or a tick
            once there are none and for a film that was watched. One badge,
            the same one the player draws, and under the pointer the button
            that marks and unmarks. */}
        <SeenMark
          watched={seen === "watched"}
          episodes={card.episodes}
          unwatched={card.unwatched}
          onPress={(watched) => marks.setWatched(card, watched)}
        />

        <div className="card-hover">
          {playable && (
            <button
              type="button"
              className="card-play"
              aria-label={t("work.play")}
              title={t("work.play")}
              onClick={stop(() => navigate(`/work/${card.id}?play`))}
            >
              <PlayIcon size={32} />
            </button>
          )}

          <div className="card-corner">
            <button
              type="button"
              className={`card-mark${favourite ? " card-mark-on" : ""}`}
              aria-pressed={favourite}
              aria-label={t(favourite ? "card.unfavourite" : "card.favourite")}
              title={t(favourite ? "card.unfavourite" : "card.favourite")}
              onClick={stop(() => marks.setFavourite(card, !favourite))}
            >
              <HeartIcon size={17} filled={favourite} />
            </button>
            <button
              ref={kebab}
              type="button"
              className={`card-mark${menuFrom ? " card-mark-on" : ""}`}
              aria-label={t("card.more")}
              title={t("card.more")}
              aria-expanded={menuFrom !== null}
              onClick={stop(() =>
                setMenuFrom((was) =>
                  was ? null : (kebab.current?.getBoundingClientRect() ?? null),
                ),
              )}
            >
              <MoreIcon size={17} />
            </button>
          </div>
        </div>

        {/* How far in this film already is, drawn on the picture itself: it is
            the one thing that tells two cards of a row apart at a glance. */}
        {howFar !== undefined && howFar > 0 && (
          <span className="card-progress" aria-hidden="true">
            <span
              className="card-progress-done"
              style={{ width: `${Math.min(howFar, 1) * 100}%` }}
            />
          </span>
        )}

      </div>

      {/* The name, and at its right what a row of half watched films is read
          for: how much of each one is left. Underneath, which episode it is,
          or the year for anything that is not one. */}
      <span className="card-line">
        <span className="card-title">{lead ?? card.title}</span>
        {trailing && <span className="card-trailing">{trailing}</span>}
      </span>
      <span className="card-year">{note ?? card.year ?? ""}</span>

      {menuFrom && (
        <CardMenu card={card} from={menuFrom} onClose={() => setMenuFrom(null)} />
      )}
    </article>
  );
}
