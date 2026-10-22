/*
 * One work as a line of a list: its picture, small, and beside it what a
 * grid shows only under the pointer or not at all.
 */

import { memo } from "react";
import { Link } from "react-router-dom";
import type { Card as CardData } from "../api";
import { StarIcon } from "../icons";
import { useMarksOf } from "../marks";
import { howLong, howMany, outOfTen, whatIsLeft } from "../readable";
import { useSettings } from "../settings";
import { lengthOfAPlay } from "../watching";
import { Card, ROOM_IN_A_LINE } from "./card";
import type { CardShape } from "./card";
import { SeenButton } from "./seen";

export const WorkLine = memo(function WorkLine({
  card,
  shape,
}: {
  card: CardData;
  /** How its picture is drawn: standing as a poster, or lying as a still. */
  shape: CardShape;
}) {
  const { t } = useSettings();
  const { resume, gone } = useMarksOf(card);
  if (gone) {
    return null;
  }
  const left = resume !== null ? whatIsLeft(resume, lengthOfAPlay(card), t) : undefined;

  return (
    <div className={`work-line work-line-${shape}`}>
      <Card card={card} shape={shape} named={false} cornered={false} room={ROOM_IN_A_LINE[shape]} />
      <div className="work-line-words">
        <div className="work-line-head">
          <Link className="work-line-name" to={`/work/${card.id}`}>
            {card.title}
          </Link>
          {card.watched_marks && <SeenButton card={card} small />}
        </div>
        <p className="work-facts work-line-facts">
          {card.year !== null && <span>{card.year}</span>}
          {card.rating !== null && (
            <span className="work-rating">
              <StarIcon size={15} />
              {outOfTen(card.rating)}
            </span>
          )}
          {card.episodes > 0 ? (
            <span>{howMany(card.episodes, "work.episode_count", t)}</span>
          ) : (
            card.runtime_minutes !== null && <span>{howLong(card.runtime_minutes, t)}</span>
          )}
          {left && <span>{left}</span>}
        </p>
      </div>
    </div>
  );
});
