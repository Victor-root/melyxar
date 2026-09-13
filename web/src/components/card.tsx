/*
 * One card in a grid.
 *
 * The colour of the poster is painted behind it before the picture arrives, so
 * a grid has colour from the first moment instead of a wall of grey holes. A
 * film nobody recognised keeps its place and wears a marker: a file set aside
 * is a file forgotten.
 */

import { Link } from "react-router-dom";
import type { Card as CardData } from "../api";
import { pictureSet } from "../api";
import { useSettings } from "../settings";

export function Card({ card, watched }: { card: CardData; watched?: number }) {
  const { t } = useSettings();
  const poster = pictureSet(card.poster);
  const unknown = card.identification === "unidentified" || card.identification === "pending";

  return (
    <Link
      className="card"
      to={`/work/${card.id}`}
      data-card
      title={card.title}
      style={{ ["--card-color" as string]: card.color ?? "var(--surface-raised)" }}
    >
      <div className="card-poster">
        {poster ? (
          <img
            src={poster.src}
            srcSet={poster.srcSet}
            sizes="(max-width: 700px) 40vw, var(--card-width)"
            alt=""
            loading="lazy"
            decoding="async"
            width={2}
            height={3}
          />
        ) : (
          <span className="card-initial" aria-hidden="true">
            {card.title.slice(0, 1)}
          </span>
        )}
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
        {card.rating !== null && <span className="card-rating">{card.rating.toFixed(1)}</span>}
        {/* How far in this film already is, drawn on the poster itself: it is
            the one thing that tells two cards of a row apart at a glance. */}
        {watched !== undefined && watched > 0 && (
          <span className="card-progress" aria-hidden="true">
            <span className="card-progress-done" style={{ width: `${watched * 100}%` }} />
          </span>
        )}
      </div>
      <span className="card-title">{card.title}</span>
      <span className="card-year">{card.year ?? ""}</span>
    </Link>
  );
}
