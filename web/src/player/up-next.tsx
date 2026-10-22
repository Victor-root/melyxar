/*
 * The next episode, announced over the end of this one as the television
 * announces it: from the start of the credits when the file says where they
 * are, from the very end otherwise. Started once the count is over, or at
 * once on its button. Cancelled, or shut with the back key, it is not
 * offered again for this episode.
 */

import { useEffect, useRef, useState } from "react";
import type { NextEpisode, PlaybackSegment } from "../api";
import { useBackClosesIt } from "../components/back-closes";
import { useShownPicture } from "../components/picture";

/** How many seconds the next episode is announced before it starts. */
export const COUNT_FROM = 10;

/** How wide the still is drawn, as the stylesheet draws it. */
const STILL_ROOM = "(max-width: 700px) 90vw, 240px";

/** Whether the next episode is offered: one there is, not turned down, and
 *  the film in its credits or over. */
export function upNextOffered(
  next: NextEpisode | null,
  declined: boolean,
  ended: boolean,
  segments: PlaybackSegment[],
  at: number,
): boolean {
  if (next === null || declined) {
    return false;
  }
  const credits = segments.find((segment) => segment.kind === "outro");
  return ended || (credits !== undefined && at >= credits.from_second);
}

type Say = (key: string, values?: Record<string, string | number>) => string;

export function UpNext({
  next,
  t,
  onPlay,
  onCancel,
}: {
  next: NextEpisode;
  t: Say;
  onPlay: () => void;
  onCancel: () => void;
}) {
  const [left, setLeft] = useState(COUNT_FROM);
  const play = useRef(onPlay);
  play.current = onPlay;
  const leave = useBackClosesIt(onCancel);
  const { picture, itDidNotLoad } = useShownPicture(next.still);

  useEffect(() => {
    const timer = window.setInterval(() => setLeft((seconds) => seconds - 1), 1000);
    return () => window.clearInterval(timer);
  }, [next.id]);

  useEffect(() => {
    if (left <= 0) {
      leave(() => play.current());
    }
  }, [left, leave]);

  const which =
    next.season !== null && next.episode !== null
      ? t("home.up_next.short", { season: next.season, episode: next.episode })
      : null;
  const name = [which, next.title].filter(Boolean).join(" · ");

  return (
    <section className="player-up-next" aria-live="polite">
      {/* Its still and what it is about, as its line on the page of the
          season shows it. */}
      <div className="player-up-next-still">
        {picture && (
          <img
            src={picture.src}
            srcSet={picture.srcSet}
            sizes={STILL_ROOM}
            alt=""
            decoding="async"
            onError={itDidNotLoad}
          />
        )}
      </div>
      <div className="player-up-next-words">
        <p className="player-up-next-label">{t("player.next_episode")}</p>
        {name && <p className="player-up-next-name">{name}</p>}
        {next.overview && <p className="player-up-next-overview">{next.overview}</p>}
        <p className="player-up-next-count">{t("player.up_next.in", { seconds: Math.max(left, 0) })}</p>
      </div>
      <div className="player-up-next-actions">
        <button type="button" className="button button-accent" onClick={() => leave(onPlay)}>
          {t("player.up_next.play_now")}
        </button>
        <button type="button" className="button" onClick={() => leave(onCancel)}>
          {t("player.up_next.cancel")}
        </button>
      </div>
    </section>
  );
}
