/*
 * What the page opens on.
 *
 * Five works at the most, shown one at a time and full width. The first is
 * what somebody left halfway, because reaching for the film you were watching
 * is what people come here to do; then what an administrator put in front of
 * everybody, then what has just arrived, then what the server offers. The
 * server decided all that when it filled the row, and says why on each one,
 * so this never works it out a second time.
 *
 * It moves on by itself, slowly, and stops the moment anybody touches it. A
 * banner that carries on sliding while somebody is reading it is a banner
 * that reads its own text out from under them, and one that slides while the
 * pointer is over its buttons is a banner that changes what a click is about
 * to do.
 *
 * It is a module, and the day the home page can be arranged by hand it is one
 * of the ones that can be turned off. Nothing here assumes it is always
 * drawn: an empty one draws nothing at all.
 */

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import { keep, recall } from "../kept";
import type { HeroItem } from "../api";
import { useShownPicture } from "./picture";
import { howLong, whatTheFileHolds, whichEpisode } from "../readable";
import { useSettings } from "../settings";
import { ChevronLeftIcon, ChevronRightIcon, InfoIcon, PlayIcon } from "../icons";

/** How many genres are named beside a title. Two: past that it is a list of
 *  everything the film could be filed under rather than what it is. */
const GENRES_NAMED = 2;

/** How long one work stands before the next takes its place. */
const EACH_STANDS_FOR = 9000;

/**
 * The fewest lines of synopsis worth drawing, and the most.
 *
 * One line, cut short, is still a synopsis beginning, so the synopsis is
 * drawn whenever a single line of it will go in. It goes only where not even
 * that fits, which is the one case where drawing it means pushing the buttons
 * off the foot of the banner. Over a dozen it stops being what the film is
 * about and becomes the whole of the page: a banner set to fill a tall screen
 * would otherwise hand over thirty lines of it.
 */
const FEWEST_LINES = 1;
const MOST_LINES = 12;

/**
 * How many whole lines fit in the room left over, and none at all when not
 * even one does.
 *
 * Whole ones: a box given the room for three lines and a third draws the
 * third one cut off along the middle of its letters, and a banner is the one
 * place on the screen where that is unmissable. The text that will not fit is
 * cut short with an ellipsis, which the stylesheet does; this only says how
 * many lines there are to cut it to.
 *
 * Exported for its own test: it is the one piece of arithmetic here whose
 * answer nobody can check by looking at the screen, since a banner that shows
 * one line too few looks exactly like a banner with a short synopsis.
 */
export function linesThatFit(room: number, lineHeight: number): number {
  if (!(lineHeight > 0)) {
    return FEWEST_LINES;
  }
  const fit = Math.floor(room / lineHeight);
  return fit < FEWEST_LINES ? 0 : Math.min(fit, MOST_LINES);
}

/**
 * What is left of the block for the synopsis: the block's own room, less each
 * of the other lines with its margins, less one gap between every two of
 * them.
 *
 * Nothing here knows about the bar at the top of the screen. The block keeps
 * clear of it with a padding of its own, so what is measured here is already
 * room the bar cannot be standing in.
 *
 * The synopsis itself is left out of the sum on purpose. That is what makes
 * the answer the same whether it is drawn long, short or not at all, and so
 * what stops the sum chasing its own result round in a circle.
 */
function roomLeftOver(box: HTMLElement, text: HTMLElement): number {
  const around = getComputedStyle(box);
  const others = (Array.from(box.children) as HTMLElement[]).filter(
    (child) => child !== text,
  );

  let room =
    box.clientHeight - parseFloat(around.paddingTop) - parseFloat(around.paddingBottom);
  for (const other of others) {
    const its = getComputedStyle(other);
    room -= other.offsetHeight + parseFloat(its.marginTop) + parseFloat(its.marginBottom);
  }
  return room - (parseFloat(around.rowGap) || 0) * others.length;
}

export function Hero({ items }: { items: HeroItem[] }) {
  const { t } = useSettings();
  /* Where the banner stood and whether a hand had stopped it, kept for this
     visit of the page: walked back to from a work opened from it, the banner
     carries on from the same picture as though nobody had left. */
  const standing = `hero-at:${useLocation().key}`;
  const [at, setAt] = useState(() => recall<number>(standing)?.value ?? 0);
  /* Set by the first touch of any kind and never unset: a banner that starts
     moving again the moment somebody looks away is worse than one that never
     moved, because it moves exactly when nobody is watching for it. */
  const [held, setHeld] = useState(() => recall<boolean>(`${standing}:held`)?.value ?? false);
  useEffect(() => {
    keep(standing, at);
    keep(`${standing}:held`, held);
  }, [standing, at, held]);
  /* The last picture asked for so far. The one in front and the one after
     it, to begin with: that is all the banner needs to move on without a
     hole, and the others would only stand in the queue in front of the
     posters of the rows, at the width of the whole screen. */
  const [farthest, setFarthest] = useState(1);
  const holder = useRef<HTMLElement>(null);
  const words = useRef<HTMLDivElement>(null);
  const synopsis = useRef<HTMLParagraphElement>(null);
  const [lines, setLines] = useState(FEWEST_LINES + 1);

  const many = items.length > 1;
  /* Nothing at all while the banner holds nothing, which is answered further
     down by drawing nothing at all. It is worked out up here because the sum
     below is redone whenever it changes. */
  const shown = items[Math.min(at, items.length - 1)];

  useEffect(() => {
    if (!many || held) {
      return;
    }
    const step = window.setInterval(
      () => setAt((was) => (was + 1) % items.length),
      EACH_STANDS_FOR,
    );
    return () => window.clearInterval(step);
  }, [many, held, items.length]);

  useEffect(() => setFarthest((was) => Math.max(was, at + 1)), [at]);

  // Somebody who asked their system for less movement is not shown a banner
  // that moves on by itself at all.
  useEffect(() => {
    const asked = window.matchMedia("(prefers-reduced-motion: reduce)");
    if (asked.matches) {
      setHeld(true);
    }
  }, []);

  /*
   * How many lines of synopsis there is room for, worked out again whenever
   * anything it depends on moves: the window, the height the banner is set
   * to, the work being shown, a drawn title arriving, a quotation that runs
   * to three lines on one work and to none on the next.
   *
   * Watching every part of the block rather than only the block itself, since
   * the block is as tall as the banner and stays that way while what is
   * inside it changes. The synopsis is watched too and costs nothing: the sum
   * leaves it out, so it lands on the same answer and stops there.
   */
  useLayoutEffect(() => {
    const box = words.current;
    const text = synopsis.current;
    if (!box || !text) {
      return;
    }
    const reckon = () =>
      setLines(
        linesThatFit(roomLeftOver(box, text), parseFloat(getComputedStyle(text).lineHeight)),
      );

    const watch = new ResizeObserver(reckon);
    watch.observe(box);
    for (const child of Array.from(box.children)) {
      watch.observe(child);
    }
    return () => watch.disconnect();
  }, [shown]);

  if (items.length === 0) {
    return null;
  }

  const go = (to: number) => {
    setHeld(true);
    setAt((to + items.length) % items.length);
  };

  return (
    <section
      className="hero"
      ref={holder}
      aria-label={t("home.hero")}
      onPointerEnter={() => setHeld(true)}
      onFocusCapture={() => setHeld(true)}
    >
      {/* Every one of them is drawn, and only the one in front is shown: the
          next picture is then already in the browser when the banner moves
          on, so it changes rather than blinking through a hole. Each is
          asked for one step ahead of being shown, and all of them the moment
          somebody takes the banner in hand, when any of them can be picked.

          All of them under one box, which is what wears the fade at the
          banner's foot. Worn by each picture instead, the same fade was
          five of them, and a browser keeps a layer of its own for every
          box it has to fade out. */}
      <div className="hero-pictures">
        {items.map((item, rank) => (
          <HeroBackdrop
            key={item.id}
            item={item}
            shown={rank === at}
            wanted={held || rank <= farthest}
          />
        ))}
      </div>

      <div
        className="hero-words"
        ref={words}
        style={{ ["--hero-lines" as string]: lines }}
      >
        {shown.tagline && <p className="hero-tagline">{shown.tagline}</p>}
        <HeroTitle item={shown} />

        {/* One line: what the film is, and then what the copy on the disk
            holds. They were two lines, and the second cost the synopsis one
            of its own, which is the line somebody actually reads. */}
        <p className="hero-facts">
          <span>
            {[
            // An episode leads with which episode it is: the title above is
            // its series, so without this nobody knows where they left off.
            whichEpisode(shown, t),
            shown.year,
            shown.runtime_minutes ? howLong(shown.runtime_minutes, t) : null,
            ...shown.genres.slice(0, GENRES_NAMED),
            shown.episodes > 0 ? t("card.unwatched", { count: shown.unwatched }) : null,
            ]
              .filter(Boolean)
              .join(" · ")}
          </span>

          {/* What the file itself holds. Beside the rest because it is what
              decides whether somebody watches this one here or on the screen
              in the other room, and because it is the one thing a catalogue
              cannot promise: it was read off the copy on the disk. */}
          <HeroBadges item={shown} />
        </p>

        {shown.overview && (
          <p
            className={`hero-overview${lines === 0 ? " hero-overview-no-room" : ""}`}
            ref={synopsis}
          >
            {shown.overview}
          </p>
        )}

        <div className="hero-buttons">
          <HeroPlay item={shown} />
          {/* A second way to the page only beside a button that plays: when
              the first one already opens it, two buttons would say the same. */}
          {opensThePage(shown) || (
            <Link className="button button-large" to={`/work/${shown.id}`}>
              <InfoIcon size={24} />
              {t("home.hero.open")}
            </Link>
          )}
        </div>

        {/* How far in it already is, under the buttons that carry on with it. */}
        <HeroProgress item={shown} />
      </div>

      {/* Where it is in the five and the two ways to move through them, in
          one place at the bottom corner. Over the picture rather than over
          the words, and never again across the middle of the banner, where
          an arrow sat between somebody and what they were reading. */}
      {many && (
        <div className="hero-steps">
          <button
            type="button"
            className="hero-step"
            aria-label={t("home.hero.previous")}
            onClick={() => go(at - 1)}
          >
            <ChevronLeftIcon size={26} />
          </button>

          <div className="hero-dots">
            {items.map((item, rank) => (
              <button
                key={item.id}
                type="button"
                className={`hero-dot${rank === at ? " hero-dot-on" : ""}`}
                aria-label={item.title}
                aria-current={rank === at}
                onClick={() => go(rank)}
              />
            ))}
          </div>

          <button
            type="button"
            className="hero-step"
            aria-label={t("home.hero.next")}
            onClick={() => go(at + 1)}
          >
            <ChevronRightIcon size={26} />
          </button>
        </div>
      )}
    </section>
  );
}

/** What the copy on the disk holds, as badges. Nothing at all for a work
 *  whose file nobody has analysed, rather than a row of empty pills. */
function HeroBadges({ item }: { item: HeroItem }) {
  const badges = whatTheFileHolds(item);

  if (badges.length === 0) {
    return null;
  }
  return (
    <span className="hero-badges">
      {badges.map((badge) => (
        <span className="hero-badge" key={badge}>
          {badge}
        </span>
      ))}
    </span>
  );
}

/**
 * How far in it already is.
 *
 * Where it stopped and how long it is, both spelled out: "one hour twelve of
 * two hours forty six" says at once how much was watched and how much is
 * left, which a bar alone never says and a count of minutes left says only
 * half of.
 */
function HeroProgress({ item }: { item: HeroItem }) {
  const { t } = useSettings();

  if (item.resume_from_seconds === null || !item.runtime_minutes) {
    return null;
  }
  const whole = item.runtime_minutes * 60;
  const done = Math.min(item.resume_from_seconds, whole);

  return (
    <p className="progress-line">
      <span className="progress-bar" aria-hidden="true">
        <span className="progress-done" style={{ width: `${(done / whole) * 100}%` }} />
      </span>
      <span>
        {t("home.hero.progress", {
          done: howLong(Math.round(done / 60), t),
          whole: howLong(item.runtime_minutes, t),
        })}
      </span>
    </p>
  );
}

/** The wide picture behind one work, or its own colour when it has none or
 *  it is not asked for yet. */
function HeroBackdrop({ item, shown, wanted }: { item: HeroItem; shown: boolean; wanted: boolean }) {
  const { picture, itDidNotLoad } = useShownPicture(item.backdrop);

  return (
    <div
      className={`hero-backdrop${shown ? " hero-backdrop-shown" : ""}`}
      style={{ ["--card-color" as string]: item.color ?? "var(--surface-raised)" }}
      aria-hidden={!shown}
    >
      {picture && wanted && (
        <img
          src={picture.src}
          srcSet={picture.srcSet}
          sizes="100vw"
          alt=""
          decoding="async"
          /* The one in front is the largest thing on the page and the first
             thing looked at; the ones behind it wait for the posters. */
          fetchPriority={shown ? "high" : "low"}
          onError={itDidNotLoad}
        />
      )}
    </div>
  );
}

/** Its title as its own designers drew it, or written out when there is
 *  none: a work nobody has looked up still has a name. An episode is named
 *  by its series, which is the only name anybody remembers it by. */
function HeroTitle({ item }: { item: HeroItem }) {
  const { picture, itDidNotLoad } = useShownPicture(item.logo);
  const name = item.series_title ?? item.title;

  if (!picture) {
    return <h2 className="hero-title">{name}</h2>;
  }
  return (
    <h2 className="hero-title hero-title-drawn">
      <img
        src={picture.src}
        srcSet={picture.srcSet}
        /* The room a drawn title really has: a box a few hundred points
           across, never the width of the window, which is what is assumed of
           a picture offered in several sizes and asked for without this. A
           fine screen still takes the larger of the two by itself. */
        sizes="340px"
        alt={name}
        decoding="async"
        onError={itDidNotLoad}
      />
    </h2>
  );
}

/** Whether the first button opens the page of the work rather than playing
 *  it: a series is opened, and so is a work with nothing on the disk. */
function opensThePage(item: HeroItem): boolean {
  return item.source === null || item.kind === "series";
}

/** Carry on, or start: what the button says comes from why the server put
 *  this work here, so the two never disagree. */
function HeroPlay({ item }: { item: HeroItem }) {
  const { t } = useSettings();
  const navigate = useNavigate();

  if (opensThePage(item)) {
    return (
      <Link className="button button-accent button-large" to={`/work/${item.id}`}>
        <PlayIcon size={30} />
        {t("home.hero.open")}
      </Link>
    );
  }
  return (
    <button
      type="button"
      className="button button-accent button-large"
      onClick={() => navigate(`/work/${item.id}?play`)}
    >
      <PlayIcon size={20} />
      {t(item.because === "started" ? "home.hero.carry_on" : "work.play")}
    </button>
  );
}
