/*
 * The next episode, announced over the end of this one as the television
 * announces it: from the start of the credits when the file says where they
 * are, from the very end otherwise. Started once the count is over, or at
 * once on its button. Cancelled, or shut with the back key, it is not
 * offered again for this episode.
 */

import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { NextEpisode, PlaybackSegment, UpNextMoment } from "../api";
import { useBackClosesIt } from "../components/back-closes";
import { useShownPicture } from "../components/picture";

/** How many seconds the next episode is announced before it starts. */
export const COUNT_FROM = 10;

/** How wide the still is drawn, as the stylesheet draws it. */
const STILL_ROOM = "(max-width: 700px) 90vw, 272px";

/** The ring the count is drawn in: its radius, and how long it is round. */
const RING = 17;
const ROUND = 2 * Math.PI * RING;

/** How much of the ring is still lit with this many seconds left: all of it
 *  at the start of the count, none once it is over. */
export function shareLeft(left: number): number {
  return Math.min(Math.max(left / COUNT_FROM, 0), 1);
}

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

/** A step of the card in Melyxar's own journal, written at the server's
 *  debug level: the way to follow a next episode that did not start. */
export function tellOfUpNext(
  source: string,
  moment: UpNextMoment,
  facts: { at_second?: number; playing?: boolean; ended?: boolean; to?: string } = {},
): void {
  api.tellTheJournal({ source, saw: "up_next", moment, ...facts }).catch(() => {
    // A line that did not arrive is not worth troubling a viewer over.
  });
}

export function UpNext({
  next,
  running,
  t,
  onPlay,
  onCancel,
  onMoment,
}: {
  next: NextEpisode;
  /** Whether the count goes on: while the film plays, or once it is over. A
      film paused holds it where it is. */
  running: boolean;
  t: Say;
  onPlay: () => void;
  onCancel: () => void;
  /** Told of each step, for the journal. */
  onMoment: (moment: UpNextMoment) => void;
}) {
  const [left, setLeft] = useState(COUNT_FROM);
  const play = useRef(onPlay);
  play.current = onPlay;
  const said = useRef(onMoment);
  said.current = onMoment;
  const leave = useBackClosesIt(() => {
    said.current("cancelled");
    onCancel();
  });

  useEffect(() => {
    said.current("offered");
  }, [next.id]);
  const { picture, itDidNotLoad } = useShownPicture(next.still);

  useEffect(() => {
    if (!running) {
      return;
    }
    const timer = window.setInterval(() => setLeft((seconds) => seconds - 1), 1000);
    return () => window.clearInterval(timer);
  }, [next.id, running]);

  useEffect(() => {
    if (left <= 0) {
      said.current("counted_out");
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
      {/* Its still with the buttons under it, and beside them what it is
          about, whole, the card growing with it rather than cutting it. The
          words keep to the top, the still and the buttons stand in the middle
          of the height the words give them. */}
      <div className="player-up-next-side">
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
        <div className="player-up-next-actions">
          <button
            type="button"
            className="button button-accent"
            onClick={() => {
              onMoment("play_pressed");
              leave(onPlay);
            }}
          >
            {t("player.up_next.play_now")}
          </button>
          <button
            type="button"
            className="button"
            onClick={() => {
              onMoment("cancelled");
              leave(onCancel);
            }}
          >
            {t("player.up_next.cancel")}
          </button>
        </div>
      </div>
      <div className="player-up-next-words">
        <div className="player-up-next-head">
          <p className="player-up-next-label">{t("player.next_episode")}</p>
          {/* The seconds left, and a ring of the accent going out with them. */}
          <span
            className="player-up-next-count"
            role="timer"
            aria-label={t("player.up_next.in", { seconds: Math.max(left, 0) })}
          >
            <svg viewBox="0 0 40 40" aria-hidden="true">
              <circle className="player-up-next-track" cx="20" cy="20" r={RING} />
              <circle
                className="player-up-next-left"
                cx="20"
                cy="20"
                r={RING}
                strokeDasharray={ROUND}
                strokeDashoffset={ROUND * (1 - shareLeft(left))}
              />
            </svg>
            <span aria-hidden="true">{Math.max(left, 0)}</span>
          </span>
        </div>
        {name && <p className="player-up-next-name">{name}</p>}
        {next.overview && <p className="player-up-next-overview">{next.overview}</p>}
      </div>
    </section>
  );
}
