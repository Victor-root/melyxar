/*
 * What is being watched, without leaving it.
 *
 * Three sheets behind one row of tabs: what the film is, where it changes
 * scene, and who is in it. All three are questions a viewer asks halfway
 * through a film ("who is that?", "how long left of this scene?"), and the
 * answer has to arrive without the film stopping or the page changing.
 *
 * It draws what the work already carries and what the plan already said.
 * Nothing here fetches anything: the screen that opened the player had the
 * film's own description in hand, and asking the server again for what is
 * already on the page is a second of black while somebody waits.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import type { Credit, PlaybackChapter, PlaybackPlan, Work } from "../api";
import { pictureSet } from "../api";
import { useDragToScroll } from "../dragging";
import { SeenMark } from "../components/seen";
import { asClock } from "./clock";
import type { Playback } from "./engine";
import { PlayIcon } from "./icons";
import { languageName } from "../languages";
import { numberOfOne, pictureName } from "../readable";
import { heightAt, Thumbnail } from "./thumbnail";

/** The sheets, in the order their tabs stand. Episodes only ever draws for a
 *  series, which is the one tab that says so itself when it opens. */
export const SHEETS = ["info", "chapters", "cast", "episodes"] as const;
export type SheetName = (typeof SHEETS)[number];

/** How far one press of the arrow carries the row along, in cards. */
const CARDS_AT_A_TIME = 4;

/** What a card and a face are while the stylesheet has not answered yet. */
const ABOUT = { card: 214, face: 148 };

/**
 * How much room each picture in the drawer really has, so the browser picks a
 * size to fit it.
 *
 * Said out loud because a picture offered in several sizes and asked for
 * without this is assumed to fill the window: the drawer then pulled the
 * largest poster held, eight hundred points across, to draw it at a hundred
 * and eighteen, while a film was streaming through the same connection.
 *
 * The lengths follow the drawer's own tokens above.
 */
const ROOM_FOR = {
  poster: "118px",
  face: `${ABOUT.face}px`,
  card: `${ABOUT.card}px`,
};

/** What a scene card looks like when the film was never read for thumbnails. */
const A_PICTURE_IS = 9 / 16;
/** What every catalogue in the world draws a face at. */
const A_FACE_IS = 3 / 2;

/** How far apart the marks stand on a film that names no scenes, in seconds. */
const A_STEP_OF = 300;

/**
 * The sizes the stylesheet was given, read off it rather than written here
 * twice.
 *
 * A thumbnail is cut out of a sheet by pixel offsets, so the drawing needs the
 * numbers and cannot take them from a class. Writing them in both places is
 * how a viewer who makes their cards larger ends up with the wrong quarter of
 * a frame of film in every one of them, so they live in the stylesheet with
 * every other size and are read from there.
 */
function useSizes(of: React.RefObject<HTMLElement | null>): typeof ABOUT {
  const [sizes, setSizes] = useState(ABOUT);
  useEffect(() => {
    const element = of.current;
    if (!element) {
      return;
    }
    const read = () => {
      const said = getComputedStyle(element);
      const value = (name: string, fallback: number) => {
        const read = Number.parseFloat(said.getPropertyValue(name));
        return Number.isFinite(read) && read > 0 ? read : fallback;
      };
      setSizes({
        card: value("--player-drawer-card", ABOUT.card),
        face: value("--player-drawer-face", ABOUT.face),
      });
    };
    read();
    window.addEventListener("resize", read);
    return () => window.removeEventListener("resize", read);
  }, [of]);
  return sizes;
}

interface Props {
  /** The film as the library describes it, handed down rather than fetched. */
  work: Work;
  plan: PlaybackPlan;
  playback: Playback;
  /** Which sheet is drawn, which is the last one read while it is shut. */
  showing: SheetName;
  /** Whether it is folded open. Shut, it stays on screen at no height. */
  open: boolean;
  /** Which language the interface is speaking, for naming the soundtrack. */
  language: string;
  t: (key: string, values?: Record<string, string | number>) => string;
  /** Steps straight to another episode, chosen from the "up next" sheet.
   *  Absent for anything that is not an episode, which is when that sheet is
   *  never shown at all. */
  onSelectEpisode?: (episode: { id: string; source_id: string | null }) => void;
}

/** How long a film runs, the way a poster says it rather than in minutes. */
function asRuntime(minutes: number, t: Props["t"]): string {
  const hours = Math.floor(minutes / 60);
  return hours > 0
    ? t("player.runtime_long", { hours, minutes: minutes % 60 })
    : t("work.minutes", { count: minutes });
}

/**
 * The one line under the title saying what is actually being played.
 *
 * What the file holds rather than what a provider says about the film: a
 * viewer reading this is asking whether they are watching the good copy, and
 * every part of it is missing on some film somewhere.
 */
function whatIsPlaying(plan: PlaybackPlan, language: string, t: Props["t"]): string {
  const picture = plan.film.picture;
  const sound = plan.audio.find((track) => track.id === plan.chosen_audio_id);
  const said = [
    picture ? pictureName(picture.width, picture.height) : null,
    picture?.hdr ? t(`facts.hdr.${picture.hdr}`) : null,
    picture?.codec.toUpperCase() ?? null,
    sound?.language ? languageName(sound.language, language) : null,
    sound?.codec.toUpperCase() ?? null,
    plan.film.sound?.channel_layout ?? null,
    sound?.is_default ? t("player.the_default") : null,
  ];
  return said.filter((part): part is string => Boolean(part)).join(" ");
}

/**
 * One sheet, drawn under the row of tabs that opens it.
 *
 * Kept on screen while it is shut rather than taken away, folded to nothing by
 * the stylesheet. That is what lets it fold open and shut rather than appear
 * and vanish: an element taken out of the page cannot be animated on its way
 * out, because by the time it would move it is gone.
 */
export function Drawer({
  work,
  plan,
  playback,
  showing,
  open,
  language,
  t,
  onSelectEpisode,
}: Props) {
  const self = useRef<HTMLDivElement>(null);
  const sizes = useSizes(self);
  return (
    <div className="player-drawer-hold" data-open={open ? "yes" : "no"} ref={self}>
      {/* Bare on purpose: this is what folds, and a box that folds to nothing
          must have nothing of its own, not even a margin. Everything the sheet
          is made of is on the panel inside it. */}
      <div className="player-drawer-fold">
        <div
          className="player-drawer"
          role="tabpanel"
          id="player-sheet"
          aria-labelledby={`player-sheet-tab-${showing}`}
        >
          {showing === "info" && (
            <About work={work} plan={plan} playback={playback} language={language} t={t} />
          )}
          {showing === "chapters" && (
            <Chapters plan={plan} playback={playback} across={sizes.card} t={t} />
          )}
          {showing === "cast" && <Cast work={work} across={sizes.face} t={t} />}
          {showing === "episodes" && (
            <Episodes
              work={work}
              across={sizes.card}
              onSelectEpisode={onSelectEpisode}
              t={t}
            />
          )}
        </div>
      </div>
    </div>
  );
}

/** What the film is: its poster, what it is rated, and what it is about. */
function About({
  work,
  plan,
  playback,
  language,
  t,
}: Pick<Props, "work" | "plan" | "playback" | "language" | "t">) {
  const poster = pictureSet(work.poster);
  const playing = whatIsPlaying(plan, language, t);
  return (
    <div className="player-drawer-about">
      {poster && (
        <img
          className="player-drawer-poster"
          src={poster.src}
          srcSet={poster.srcSet}
          sizes={ROOM_FOR.poster}
          alt=""
        />
      )}

      <div className="player-drawer-said">
        <h2 className="player-drawer-title">{work.title}</h2>

        <p className="player-drawer-facts">
          {work.rating !== null && (
            <span className="player-drawer-rating">
              <span className="player-drawer-star" aria-hidden="true">
                ★
              </span>
              {work.rating.toFixed(1)}
            </span>
          )}
          {work.year !== null && <span>{work.year}</span>}
          {work.runtime_minutes !== null && <span>{asRuntime(work.runtime_minutes, t)}</span>}
          {work.age_rating && <span className="player-drawer-badge">{work.age_rating}</span>}
          {/* Said as a box like the age rating because it is the same kind of
              fact: something this copy of the film has, not an opinion of it. */}
          {plan.subtitles.length > 0 && <span className="player-drawer-badge">CC</span>}
        </p>

        {playing && <p className="player-drawer-playing">{playing}</p>}
        {work.overview && <p className="player-drawer-overview">{work.overview}</p>}
      </div>

      <button className="player-drawer-again" onClick={() => playback.goTo(0)}>
        <PlayIcon size={18} />
        {t("player.from_the_start")}
      </button>
    </div>
  );
}

/** Where the film changes scene, as a row of cards a hand can run along. */
function Chapters({
  plan,
  playback,
  across,
  t,
}: Pick<Props, "plan" | "playback" | "t"> & { across: number }) {
  const chapters =
    plan.chapters.length > 0 ? plan.chapters : everyFewMinutes(playback.length);
  if (chapters.length === 0) {
    return <p className="player-drawer-nothing">{t("player.no_chapters")}</p>;
  }
  const inside = whichChapter(chapters, playback.at);
  /* The frame is exactly as tall as the thumbnails really are, so a film in
     scope does not sit in a widescreen box with grey above and below it. A
     film nobody has read for thumbnails keeps a widescreen frame, which is
     what an empty scene card should look like. */
  const down = plan.thumbnails
    ? heightAt(plan.thumbnails, across)
    : Math.round(across * A_PICTURE_IS);
  return (
    <Strip along={across} picture={down}>
      {chapters.map((chapter, index) => (
        <button
          key={chapter.at_second}
          className={`player-drawer-chapter${index === inside ? " player-drawer-chapter-now" : ""}`}
          onClick={() => playback.goTo(chapter.at_second)}
        >
          <span className="player-drawer-frame" style={{ height: `${down}px` }}>
            <Thumbnail
              thumbnails={plan.thumbnails}
              seconds={chapter.at_second}
              across={across}
              className="player-drawer-still"
            />
            {/* On the picture rather than under the card. Written underneath,
                it made the one card carrying it taller than all the others,
                and a row sizes itself on its tallest card: every other card
                in the row then carried that line's worth of nothing, for a
                card that is usually scrolled out of sight. */}
            {index === inside && (
              <span className="player-drawer-card-now">{t("player.playing_now")}</span>
            )}
          </span>
          <span className="player-drawer-card-name">
            {chapter.title ?? t("player.chapter_number", { number: index + 1 })}
          </span>
          <span className="player-drawer-card-under">{asClock(chapter.at_second)}</span>
        </button>
      ))}
    </Strip>
  );
}

/**
 * Marks every few minutes, for a film whose file names no scenes.
 *
 * Most files carry none: naming scenes is something a disc does and something
 * most copies lose. The row is still the quickest way through a film, so it is
 * drawn at a regular step rather than left empty, which is what the players
 * the maintainer already uses do.
 *
 * Only here. The bar itself keeps its marks honest: a white line across it
 * says the film really changes scene there, and filling it with lines every
 * five minutes would say something about the film that is not true.
 */
function everyFewMinutes(length: number): PlaybackChapter[] {
  if (!Number.isFinite(length) || length <= A_STEP_OF) {
    return [];
  }
  const marks: PlaybackChapter[] = [];
  for (let at = 0; at < length; at += A_STEP_OF) {
    marks.push({ at_second: at, title: null });
  }
  return marks;
}

/** Which chapter a moment falls inside, or none at all before the first. */
function whichChapter(chapters: PlaybackChapter[], at: number): number {
  let inside = -1;
  for (let index = 0; index < chapters.length; index += 1) {
    if (chapters[index].at_second <= at + 0.25) {
      inside = index;
    }
  }
  return inside;
}

/** Who is in it and who made it, faces first. */
function Cast({ work, across, t }: Pick<Props, "work" | "t"> & { across: number }) {
  const everyone: Credit[] = [...work.cast, ...work.crew];
  if (everyone.length === 0) {
    return <p className="player-drawer-nothing">{t("player.nobody_named")}</p>;
  }
  return (
    <Strip along={across} picture={Math.round(across * A_FACE_IS)}>
      {everyone.map((credit, index) => {
        const photo = pictureSet(credit.photo);
        return (
          <div className="player-drawer-person" key={`${credit.name}-${credit.role}-${index}`}>
            <span className="player-drawer-face">
              {photo ? (
                <img
                  src={photo.src}
                  srcSet={photo.srcSet}
                  sizes={ROOM_FOR.face}
                  alt=""
                  loading="lazy"
                  draggable={false}
                />
              ) : (
                /* An initial rather than an empty frame: a row of grey boxes
                   reads as a page that failed to load. */
                <span className="player-drawer-initial" aria-hidden="true">
                  {credit.name.slice(0, 1)}
                </span>
              )}
            </span>
            <span className="player-drawer-card-name">{credit.name}</span>
            <span className="player-drawer-card-under">
              {credit.character ?? t(`credit.${credit.role}`)}
            </span>
          </div>
        );
      })}
    </Strip>
  );
}

/**
 * Every episode of the season this one belongs to, in order, with the one
 * playing now told apart from the rest.
 *
 * The episodes sitting beside this one travel with its own description, so
 * the sheet has them the moment it opens and asks nothing of its own.
 */
function Episodes({
  work,
  across,
  onSelectEpisode,
  t,
}: Pick<Props, "work" | "t" | "onSelectEpisode"> & { across: number }) {
  const episodes = work.siblings;

  if (work.kind !== "episode") {
    return null;
  }
  if (episodes.length === 0) {
    return <p className="player-drawer-nothing">{t("player.no_episodes")}</p>;
  }

  const down = Math.round(across * A_PICTURE_IS);
  return (
    <Strip along={across} picture={down}>
      {episodes.map(({ card, number, title }) => {
        const poster = pictureSet(card.poster);
        const now = card.id === work.id;
        const canPlay = !now && card.source !== null;
        return (
          <button
            key={card.id}
            className={`player-drawer-episode${now ? " player-drawer-episode-now" : ""}`}
            disabled={!canPlay}
            onClick={
              canPlay ? () => onSelectEpisode?.({ id: card.id, source_id: card.source }) : undefined
            }
          >
            <span className="player-drawer-frame" style={{ height: `${down}px` }}>
              {poster ? (
                <img
                  src={poster.src}
                  srcSet={poster.srcSet}
                  sizes={ROOM_FOR.card}
                  alt=""
                  loading="lazy"
                  draggable={false}
                />
              ) : (
                <span className="player-drawer-initial" aria-hidden="true">
                  {number ?? ""}
                </span>
              )}
              {now && <span className="player-drawer-card-now">{t("player.playing_now")}</span>}
              {card.seen === "watched" && <SeenMark watched />}
            </span>
            <span className="player-drawer-card-name">{numberOfOne("episode", number, t)}</span>
            {title && <span className="player-drawer-card-title">{title}</span>}
            <span className="player-drawer-card-under">
              {card.source === null
                ? t("work.not_on_disk")
                : card.runtime_minutes
                  ? t("work.minutes", { count: card.runtime_minutes })
                  : ""}
            </span>
          </button>
        );
      })}
    </Strip>
  );
}

/**
 * A row that runs off the side, with an arrow at whichever end has more.
 *
 * Scrolled rather than paged: a hand on a trackpad or a touchscreen already
 * knows how to move it. The arrows are for the one case that does not, a mouse
 * with a wheel that only goes up and down, and each shows only while there is
 * something that way to reach.
 */
function Strip({
  along: card,
  picture,
  children,
}: {
  /** How wide one card is, which is how far a press of the arrow steps. */
  along: number;
  /** How tall the picture on a card is, which is where the arrows sit: level
   *  with it rather than with the words underneath. */
  picture: number;
  children: React.ReactNode;
}) {
  const row = useRef<HTMLDivElement>(null);
  const [more, setMore] = useState({ back: false, on: false });

  const look = useCallback(() => {
    const element = row.current;
    if (!element) {
      return;
    }
    // A pixel of slack: a row scrolled to its very end lands a fraction short
    // of its own width often enough for an arrow to stay lit for ever.
    setMore({
      back: element.scrollLeft > 1,
      on: element.scrollLeft + element.clientWidth < element.scrollWidth - 1,
    });
  }, []);

  useEffect(() => {
    look();
    const element = row.current;
    if (!element || typeof ResizeObserver === "undefined") {
      return;
    }
    const watching = new ResizeObserver(look);
    watching.observe(element);
    return () => watching.disconnect();
  }, [look, children]);

  const along = (by: number) =>
    row.current?.scrollBy({ left: by * CARDS_AT_A_TIME * card, behavior: "smooth" });

  // Held down and pulled sideways. The same one every sideways row in this
  // interface uses, this one included: it was written here first.
  const drag = useDragToScroll(row);

  return (
    <div
      className="player-drawer-strip"
      style={{ ["--player-drawer-picture" as string]: `${picture}px` }}
    >
      <div
        className="player-drawer-row"
        ref={row}
        onScroll={look}
        {...drag}
      >
        {children}
      </div>
      {more.back && (
        <button
          className="player-drawer-along player-drawer-along-back"
          onClick={() => along(-1)}
          aria-hidden="true"
          tabIndex={-1}
        >
          {"‹"}
        </button>
      )}
      {more.on && (
        <button
          className="player-drawer-along player-drawer-along-on"
          onClick={() => along(1)}
          aria-hidden="true"
          tabIndex={-1}
        >
          {"›"}
        </button>
      )}
    </div>
  );
}
